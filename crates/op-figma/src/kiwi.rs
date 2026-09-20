//! Kiwi schema decoder — ports the subset of Evan Wallace's
//! `kiwi-schema` that a `.fig` file needs: `ByteBuffer` wire reads,
//! `decodeBinarySchema` (the self-describing schema chunk), and a
//! dynamic message decoder that turns the data chunk into a
//! [`FigValue`] tree (the equivalent of `compileSchema` + `decode*`).
//!
//! `.fig` files carry two Kiwi parts: part 0 is a binary schema, part 1
//! is data encoded against that schema. The schema names the root
//! message type `Message`, so [`decode_message`] decodes part 1 against
//! the `Message` definition.

use std::collections::HashMap;
use std::sync::Arc;

/// Native Kiwi types, indexed by `!encoded_type` (bitwise-NOT) for the
/// negative type codes in a binary schema.
const NATIVE_TYPES: [&str; 8] = [
    "bool", "byte", "int", "uint", "float", "string", "int64", "uint64",
];

/// Recursion ceiling for nested message/struct decoding — guards
/// against a hostile schema that defines cyclic message nesting.
const MAX_DEPTH: u32 = 256;

#[derive(Debug)]
pub enum KiwiError {
    /// Read past the end of the buffer.
    OutOfBounds,
    /// Schema type index / native code is out of range.
    BadType(String),
    /// Data references a message field id absent from the schema.
    UnknownField(String),
    /// No `Message` (or any message) definition in the schema.
    NoRootMessage,
    /// Nested decoding exceeded [`MAX_DEPTH`].
    TooDeep,
}

impl std::fmt::Display for KiwiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KiwiError::OutOfBounds => write!(f, "kiwi: read past end of buffer"),
            KiwiError::BadType(s) => write!(f, "kiwi: bad type reference: {s}"),
            KiwiError::UnknownField(s) => write!(f, "kiwi: unknown message field: {s}"),
            KiwiError::NoRootMessage => write!(f, "kiwi: schema has no message definition"),
            KiwiError::TooDeep => write!(f, "kiwi: nesting too deep"),
        }
    }
}

// ── ByteBuffer ────────────────────────────────────────────────────

/// Cursor over a Kiwi wire buffer. All multi-byte integers are
/// LEB128 varints; floats use Kiwi's exponent-rotated encoding.
pub struct ByteBuffer<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> ByteBuffer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ByteBuffer { data, index: 0 }
    }

    fn at_end(&self) -> bool {
        self.index >= self.data.len()
    }

    /// Bytes left to read from the current cursor.
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.index)
    }

    pub fn read_byte(&mut self) -> Result<u8, KiwiError> {
        let b = *self.data.get(self.index).ok_or(KiwiError::OutOfBounds)?;
        self.index += 1;
        Ok(b)
    }

    /// LEB128 unsigned varint (max 5 bytes → 32 bits).
    pub fn read_var_uint(&mut self) -> Result<u32, KiwiError> {
        let mut value: u32 = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_byte()?;
            value |= ((byte & 127) as u32).wrapping_shl(shift);
            shift += 7;
            if byte & 128 == 0 || shift >= 35 {
                break;
            }
        }
        Ok(value)
    }

    /// Zigzag-decoded signed varint.
    pub fn read_var_int(&mut self) -> Result<i32, KiwiError> {
        let v = self.read_var_uint()?;
        Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
    }

    /// Kiwi 64-bit unsigned varint: eight 7-bit groups (56 bits) then a
    /// final byte contributing all 8 bits — matching `kiwi-schema`.
    pub fn read_var_uint64(&mut self) -> Result<u64, KiwiError> {
        let mut value: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_byte()?;
            if shift < 56 {
                value |= ((byte & 127) as u64).wrapping_shl(shift);
                shift += 7;
                if byte & 128 == 0 {
                    break;
                }
            } else {
                // Final byte — all 8 bits, no continuation.
                value |= (byte as u64).wrapping_shl(shift);
                break;
            }
        }
        Ok(value)
    }

    /// Zigzag-decoded signed 64-bit varint.
    pub fn read_var_int64(&mut self) -> Result<i64, KiwiError> {
        let v = self.read_var_uint64()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    /// Kiwi var-float: a single `0` byte encodes `0.0`; otherwise a
    /// 4-byte little-endian word whose exponent has been rotated into
    /// the low 8 bits (`rotate_left(23)` restores the IEEE-754 bits).
    pub fn read_var_float(&mut self) -> Result<f32, KiwiError> {
        let first = *self.data.get(self.index).ok_or(KiwiError::OutOfBounds)?;
        if first == 0 {
            self.index += 1;
            return Ok(0.0);
        }
        let raw = self
            .data
            .get(self.index..self.index + 4)
            .ok_or(KiwiError::OutOfBounds)?;
        let stored = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        self.index += 4;
        Ok(f32::from_bits(stored.rotate_left(23)))
    }

    /// NUL-terminated UTF-8 string.
    pub fn read_string(&mut self) -> Result<String, KiwiError> {
        let start = self.index;
        while !self.at_end() && self.data[self.index] != 0 {
            self.index += 1;
        }
        if self.at_end() {
            return Err(KiwiError::OutOfBounds);
        }
        let s = String::from_utf8_lossy(&self.data[start..self.index]).into_owned();
        self.index += 1; // consume the NUL
        Ok(s)
    }

    /// Length-prefixed raw byte block (`byte[]` fields).
    pub fn read_byte_array(&mut self) -> Result<Vec<u8>, KiwiError> {
        let len = self.read_var_uint()? as usize;
        let slice = self
            .data
            .get(self.index..self.index + len)
            .ok_or(KiwiError::OutOfBounds)?;
        self.index += len;
        Ok(slice.to_vec())
    }
}

// ── Schema ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Enum,
    Struct,
    Message,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    /// Resolved type: a native name (`int`, `string`, …) or another
    /// definition's name.
    pub type_name: String,
    pub is_array: bool,
    /// Enum: the integer value. Message: the wire field id.
    pub value: u32,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub kind: DefKind,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub definitions: Vec<Definition>,
}

/// One raw field before type resolution: `(name, type_code, is_array,
/// value)`. Type codes stay numeric until every definition is known.
type RawField = (String, i32, bool, u32);
/// One raw definition before type resolution: `(name, kind, fields)`.
type RawDef = (String, DefKind, Vec<RawField>);

/// Decode the self-describing binary schema chunk
/// (`decodeBinarySchema`). Type codes are bound to names: a negative
/// code is a native type (`!code` indexes [`NATIVE_TYPES`]), a
/// non-negative code indexes the definition list.
pub fn decode_binary_schema(bytes: &[u8]) -> Result<Schema, KiwiError> {
    let mut bb = ByteBuffer::new(bytes);
    let definition_count = bb.read_var_uint()?;
    // Raw types are kept as ints until every definition is known, so
    // forward references resolve.
    let mut raw: Vec<RawDef> = Vec::new();
    for _ in 0..definition_count {
        let name = bb.read_string()?;
        let kind = match bb.read_byte()? {
            0 => DefKind::Enum,
            1 => DefKind::Struct,
            2 => DefKind::Message,
            other => return Err(KiwiError::BadType(format!("definition kind {other}"))),
        };
        let field_count = bb.read_var_uint()?;
        // Each field encodes at least a few bytes — cap the pre-alloc
        // at the remaining buffer size so a hostile count can't force
        // a huge allocation before the read loop fails naturally.
        let mut fields = Vec::with_capacity((field_count as usize).min(bb.remaining()));
        for _ in 0..field_count {
            let field_name = bb.read_string()?;
            let type_code = bb.read_var_int()?;
            let is_array = bb.read_byte()? & 1 != 0;
            let value = bb.read_var_uint()?;
            fields.push((field_name, type_code, is_array, value));
        }
        raw.push((name, kind, fields));
    }

    let names: Vec<String> = raw.iter().map(|(n, _, _)| n.clone()).collect();
    let mut definitions = Vec::with_capacity(raw.len());
    for (name, kind, fields) in raw {
        let mut resolved = Vec::with_capacity(fields.len());
        for (field_name, type_code, is_array, value) in fields {
            // Enum fields carry only a name + ordinal; their encoded
            // type code is unused (kiwi writes 0). Skip resolution so
            // a stray code never rejects an otherwise-valid schema.
            let type_name = if kind == DefKind::Enum {
                String::new()
            } else if type_code < 0 {
                let idx = !type_code;
                NATIVE_TYPES
                    .get(idx as usize)
                    .ok_or_else(|| KiwiError::BadType(format!("native code {type_code}")))?
                    .to_string()
            } else {
                names
                    .get(type_code as usize)
                    .ok_or_else(|| KiwiError::BadType(format!("definition index {type_code}")))?
                    .clone()
            };
            resolved.push(Field {
                name: field_name,
                type_name,
                is_array,
                value,
            });
        }
        definitions.push(Definition {
            name,
            kind,
            fields: resolved,
        });
    }
    Ok(Schema { definitions })
}

// ── FigValue ──────────────────────────────────────────────────────

/// Dynamically-typed decoded value — the Rust equivalent of the
/// untyped `any` tree the TS parser works with.
#[derive(Debug, Clone, PartialEq)]
pub enum FigValue {
    /// Absent / unresolved (e.g. an unknown enum ordinal).
    Null,
    Bool(bool),
    Int(i32),
    Uint(u32),
    Int64(i64),
    Uint64(u64),
    Float(f32),
    Str(String),
    Bytes(Vec<u8>),
    Array(Vec<FigValue>),
    /// Insertion-ordered key/value pairs (Kiwi struct/message). Keys decoded
    /// from the schema share one allocation across every object instance.
    Object(Vec<(Arc<str>, FigValue)>),
}

impl FigValue {
    /// Field lookup on an object value; `None` for non-objects, absent
    /// keys, or keys explicitly decoded to [`FigValue::Null`].
    pub fn get(&self, key: &str) -> Option<&FigValue> {
        match self {
            FigValue::Object(pairs) => pairs
                .iter()
                .find(|(k, _)| k.as_ref() == key)
                .map(|(_, v)| v)
                .filter(|v| !matches!(v, FigValue::Null)),
            _ => None,
        }
    }

    /// Mutable field lookup on an object value. Explicit `Null`
    /// remains absent, matching [`get`](Self::get).
    pub fn get_mut(&mut self, key: &str) -> Option<&mut FigValue> {
        match self {
            FigValue::Object(pairs) => pairs
                .iter_mut()
                .find(|(name, _)| name.as_ref() == key)
                .map(|(_, value)| value)
                .filter(|value| !matches!(value, FigValue::Null)),
            _ => None,
        }
    }

    /// Numeric coercion across every integer/float variant.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            FigValue::Int(v) => Some(*v as f64),
            FigValue::Uint(v) => Some(*v as f64),
            FigValue::Int64(v) => Some(*v as f64),
            FigValue::Uint64(v) => Some(*v as f64),
            FigValue::Float(v) => Some(*v as f64),
            FigValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            FigValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[FigValue]> {
        match self {
            FigValue::Array(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            FigValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, FigValue::Object(_))
    }

    /// Numeric field read (`obj.key` coerced to f64).
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    /// String field read.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Boolean field read.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Array field read.
    pub fn get_array(&self, key: &str) -> Option<&[FigValue]> {
        self.get(key).and_then(|v| v.as_array())
    }

    /// Mutable array field read.
    pub fn get_array_mut(&mut self, key: &str) -> Option<&mut Vec<FigValue>> {
        match self.get_mut(key)? {
            FigValue::Array(values) => Some(values),
            _ => None,
        }
    }

    /// Insert or overwrite a field on an object value (no-op for
    /// non-objects).
    pub fn set(&mut self, key: &str, value: FigValue) {
        if let FigValue::Object(pairs) = self {
            if let Some(slot) = pairs.iter_mut().find(|(k, _)| k.as_ref() == key) {
                slot.1 = value;
            } else {
                pairs.push((Arc::from(key), value));
            }
        }
    }
}

// ── Dynamic decoder ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeType {
    Bool,
    Byte,
    Int,
    Uint,
    Float,
    String,
    Int64,
    Uint64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedType {
    Native(NativeType),
    Definition(usize),
}

struct Decoder<'a, 'b> {
    schema: &'a Schema,
    bb: ByteBuffer<'b>,
    /// Resolved field types parallel to `Schema::definitions[*].fields`.
    /// Enum fields have no wire type and therefore store `None`.
    field_types: Vec<Vec<Option<ResolvedType>>>,
    /// Message/enum wire value → field index, parallel to definitions.
    field_by_value: Vec<HashMap<u32, usize>>,
    /// Object keys parallel to `Schema::definitions[*].fields`. A decoded
    /// struct can occur hundreds of thousands of times, so cloning these
    /// `Arc`s avoids allocating a fresh `String` for every occurrence.
    object_keys: Vec<Vec<Arc<str>>>,
}

impl<'a, 'b> Decoder<'a, 'b> {
    fn new(schema: &'a Schema, data: &'b [u8]) -> Result<Self, KiwiError> {
        let definition_by_name: HashMap<&str, usize> = schema
            .definitions
            .iter()
            .enumerate()
            .map(|(index, def)| (def.name.as_str(), index))
            .collect();

        let resolve = |type_name: &str| -> Result<ResolvedType, KiwiError> {
            let native = match type_name {
                "bool" => Some(NativeType::Bool),
                "byte" => Some(NativeType::Byte),
                "int" => Some(NativeType::Int),
                "uint" => Some(NativeType::Uint),
                "float" => Some(NativeType::Float),
                "string" => Some(NativeType::String),
                "int64" => Some(NativeType::Int64),
                "uint64" => Some(NativeType::Uint64),
                _ => None,
            };
            if let Some(native) = native {
                return Ok(ResolvedType::Native(native));
            }
            definition_by_name
                .get(type_name)
                .copied()
                .map(ResolvedType::Definition)
                .ok_or_else(|| KiwiError::BadType(type_name.to_string()))
        };

        let mut field_types = Vec::with_capacity(schema.definitions.len());
        let mut field_by_value = Vec::with_capacity(schema.definitions.len());
        let mut object_keys = Vec::with_capacity(schema.definitions.len());
        let mut interned_keys: HashMap<&str, Arc<str>> = HashMap::new();
        for def in &schema.definitions {
            let mut types = Vec::with_capacity(def.fields.len());
            let mut values = HashMap::with_capacity(def.fields.len());
            let mut keys = Vec::with_capacity(def.fields.len());
            for (field_index, field) in def.fields.iter().enumerate() {
                types.push(if def.kind == DefKind::Enum {
                    None
                } else {
                    Some(resolve(&field.type_name)?)
                });
                values.insert(field.value, field_index);
                let key = interned_keys
                    .entry(field.name.as_str())
                    .or_insert_with(|| Arc::from(field.name.as_str()));
                keys.push(Arc::clone(key));
            }
            field_types.push(types);
            field_by_value.push(values);
            object_keys.push(keys);
        }

        Ok(Self {
            schema,
            bb: ByteBuffer::new(data),
            field_types,
            field_by_value,
            object_keys,
        })
    }

    /// Decode one value of an already-resolved type.
    fn decode_type(&mut self, resolved: ResolvedType, depth: u32) -> Result<FigValue, KiwiError> {
        if depth > MAX_DEPTH {
            return Err(KiwiError::TooDeep);
        }
        match resolved {
            ResolvedType::Native(NativeType::Bool) => Ok(FigValue::Bool(self.bb.read_byte()? != 0)),
            ResolvedType::Native(NativeType::Byte) => {
                Ok(FigValue::Uint(self.bb.read_byte()? as u32))
            }
            ResolvedType::Native(NativeType::Int) => Ok(FigValue::Int(self.bb.read_var_int()?)),
            ResolvedType::Native(NativeType::Uint) => Ok(FigValue::Uint(self.bb.read_var_uint()?)),
            ResolvedType::Native(NativeType::Float) => {
                Ok(FigValue::Float(self.bb.read_var_float()?))
            }
            ResolvedType::Native(NativeType::String) => Ok(FigValue::Str(self.bb.read_string()?)),
            ResolvedType::Native(NativeType::Int64) => {
                Ok(FigValue::Int64(self.bb.read_var_int64()?))
            }
            ResolvedType::Native(NativeType::Uint64) => {
                Ok(FigValue::Uint64(self.bb.read_var_uint64()?))
            }
            ResolvedType::Definition(def_index) => match self.schema.definitions[def_index].kind {
                DefKind::Enum => self.decode_enum(def_index),
                DefKind::Struct => self.decode_struct(def_index, depth),
                DefKind::Message => self.decode_message_def(def_index, depth),
            },
        }
    }

    /// Decode a field — handles `byte[]` (raw block), other arrays
    /// (length-prefixed elements), and scalars.
    fn decode_field(
        &mut self,
        resolved: ResolvedType,
        is_array: bool,
        depth: u32,
    ) -> Result<FigValue, KiwiError> {
        if is_array {
            if resolved == ResolvedType::Native(NativeType::Byte) {
                return Ok(FigValue::Bytes(self.bb.read_byte_array()?));
            }
            let n = self.bb.read_var_uint()?;
            // A real array cannot have more elements than there are
            // bytes left to read — rejects a hostile length that would
            // spin the decode loop billions of times (e.g. an array of
            // a zero-byte struct type).
            if n as usize > self.bb.remaining() {
                return Err(KiwiError::OutOfBounds);
            }
            let mut items = Vec::with_capacity((n as usize).min(self.bb.remaining()));
            for _ in 0..n {
                items.push(self.decode_type(resolved, depth + 1)?);
            }
            Ok(FigValue::Array(items))
        } else {
            self.decode_type(resolved, depth + 1)
        }
    }

    /// Enum: a single varuint resolved to its field name.
    fn decode_enum(&mut self, def_index: usize) -> Result<FigValue, KiwiError> {
        let v = self.bb.read_var_uint()?;
        Ok(self.field_by_value[def_index]
            .get(&v)
            .map(|field_index| {
                FigValue::Str(
                    self.schema.definitions[def_index].fields[*field_index]
                        .name
                        .clone(),
                )
            })
            .unwrap_or(FigValue::Null))
    }

    /// Struct: every field in declared order.
    fn decode_struct(&mut self, def_index: usize, depth: u32) -> Result<FigValue, KiwiError> {
        let field_count = self.schema.definitions[def_index].fields.len();
        let mut pairs = Vec::with_capacity(field_count);
        for field_index in 0..field_count {
            let (name, resolved, is_array) = {
                let field = &self.schema.definitions[def_index].fields[field_index];
                let resolved = self.field_types[def_index][field_index]
                    .ok_or_else(|| KiwiError::BadType(field.type_name.clone()))?;
                (
                    Arc::clone(&self.object_keys[def_index][field_index]),
                    resolved,
                    field.is_array,
                )
            };
            let value = self.decode_field(resolved, is_array, depth)?;
            pairs.push((name, value));
        }
        Ok(FigValue::Object(pairs))
    }

    /// Message: id-prefixed fields, terminated by id `0`.
    fn decode_message_def(&mut self, def_index: usize, depth: u32) -> Result<FigValue, KiwiError> {
        let field_count = self.schema.definitions[def_index].fields.len();
        let mut pairs: Vec<(Arc<str>, FigValue)> = Vec::new();
        let mut pair_slots: Vec<Option<usize>> = vec![None; field_count];
        loop {
            let field_id = self.bb.read_var_uint()?;
            if field_id == 0 {
                return Ok(FigValue::Object(pairs));
            }
            let field_index = self.field_by_value[def_index]
                .get(&field_id)
                .copied()
                .ok_or_else(|| {
                    KiwiError::UnknownField(format!(
                        "{} id {field_id}",
                        self.schema.definitions[def_index].name
                    ))
                })?;
            let (resolved, is_array) = {
                let field = &self.schema.definitions[def_index].fields[field_index];
                let resolved = self.field_types[def_index][field_index]
                    .ok_or_else(|| KiwiError::BadType(field.type_name.clone()))?;
                (resolved, field.is_array)
            };
            let value = self.decode_field(resolved, is_array, depth)?;
            // A repeated id overwrites — matches the JS `result.x = …`.
            if let Some(slot) = pair_slots[field_index] {
                pairs[slot].1 = value;
            } else {
                let field_name = Arc::clone(&self.object_keys[def_index][field_index]);
                pair_slots[field_index] = Some(pairs.len());
                pairs.push((field_name, value));
            }
        }
    }
}

/// Decode the data chunk against `schema`, rooted at the `Message`
/// definition (falling back to the first message definition).
pub fn decode_message(schema: &Schema, data: &[u8]) -> Result<FigValue, KiwiError> {
    let root_index = schema
        .definitions
        .iter()
        .position(|d| d.name == "Message" && d.kind == DefKind::Message)
        .or_else(|| {
            schema
                .definitions
                .iter()
                .position(|d| d.kind == DefKind::Message)
        })
        .ok_or(KiwiError::NoRootMessage)?;
    let mut decoder = Decoder::new(schema, data)?;
    decoder.decode_message_def(root_index, 0)
}

#[cfg(test)]
#[path = "kiwi/tests.rs"]
mod tests;

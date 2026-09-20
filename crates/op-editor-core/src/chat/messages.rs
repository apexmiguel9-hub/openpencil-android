//! Per-message card state on [`ChatState`]: thinking / tool-call /
//! design-block / action-step expansion, subtask retry, the per-turn
//! selectors and staged attachments.

use super::*;

impl ChatState {
    /// Flip the collapsed state of message `idx`'s thinking block.
    /// Out-of-range index is a no-op.
    pub fn toggle_message_thinking(&mut self, idx: usize) {
        if let Some(msg) = self.messages.get_mut(idx) {
            msg.thinking_collapsed = !msg.thinking_collapsed;
        }
    }

    /// Flip the collapsed state of message `idx`'s tool-calls panel.
    /// Out-of-range index is a no-op.
    pub fn toggle_message_tool_calls(&mut self, idx: usize) {
        if let Some(msg) = self.messages.get_mut(idx) {
            msg.tools_collapsed = !msg.tools_collapsed;
        }
    }

    /// Set one tool card's expanded override. Out-of-range message /
    /// tool indexes are no-ops.
    pub fn set_message_tool_call_expanded(
        &mut self,
        msg_idx: usize,
        tool_idx: usize,
        expanded: bool,
    ) {
        let Some(msg) = self.messages.get_mut(msg_idx) else {
            return;
        };
        if tool_idx >= msg.tool_calls.len() {
            return;
        }
        if msg.tool_call_expanded_overrides.len() <= tool_idx {
            msg.tool_call_expanded_overrides.resize(tool_idx + 1, None);
        }
        msg.tool_call_expanded_overrides[tool_idx] = Some(expanded);
    }

    /// Set one design JSON card's expanded override. Out-of-range
    /// message indexes are no-ops.
    pub fn set_message_design_block_expanded(
        &mut self,
        msg_idx: usize,
        block_idx: usize,
        expanded: bool,
    ) {
        let Some(msg) = self.messages.get_mut(msg_idx) else {
            return;
        };
        if msg.design_block_expanded_overrides.len() <= block_idx {
            msg.design_block_expanded_overrides
                .resize(block_idx + 1, None);
        }
        msg.design_block_expanded_overrides[block_idx] = Some(expanded);
    }

    /// Set one action-step (subtask) card's expanded override. Out-of-range
    /// message indexes are no-ops.
    pub fn set_message_action_step_expanded(
        &mut self,
        msg_idx: usize,
        step_idx: usize,
        expanded: bool,
    ) {
        let Some(msg) = self.messages.get_mut(msg_idx) else {
            return;
        };
        if msg.action_step_expanded_overrides.len() <= step_idx {
            msg.action_step_expanded_overrides
                .resize(step_idx + 1, None);
        }
        msg.action_step_expanded_overrides[step_idx] = Some(expanded);
    }

    /// Begin a manual retry for the failed subtask row at
    /// `activities[source_index]` in message `msg_idx` — the click handler
    /// for the progress panel's per-row "Retry" button. Flips that
    /// activity's status back to `Running` and clears its stale "Needs
    /// attention" detail so the row shows a spinner immediately, then
    /// raises `pending_subtask_retry` for the desktop host to drain.
    ///
    /// No-ops (leaves everything untouched) when the message/activity index
    /// is out of range, or when that activity has no persisted
    /// [`PendingSubtaskRetry`] entry — a row with nothing to retry (e.g. it
    /// never actually failed) must not silently start a phantom turn.
    pub fn begin_subtask_retry(&mut self, msg_idx: usize, source_index: usize) {
        let Some(msg) = self.messages.get_mut(msg_idx) else {
            return;
        };
        let Some(subtask_id) = msg.activities.get(source_index).map(|a| a.id.clone()) else {
            return;
        };
        if !msg
            .failed_subtasks
            .iter()
            .any(|p| p.subtask_id == subtask_id)
        {
            return;
        }
        if let Some(activity) = msg.activities.get_mut(source_index) {
            activity.status = ChatActivityStatus::Running;
            activity.detail = None;
        }
        self.pending_subtask_retry = Some((msg_idx, subtask_id));
    }

    /// Advance the thinking-mode selector one step:
    /// Adaptive → Disabled → Enabled → Adaptive.
    pub fn cycle_thinking_mode(&mut self) {
        self.thinking_mode = match self.thinking_mode {
            ThinkingMode::Adaptive => ThinkingMode::Disabled,
            ThinkingMode::Disabled => ThinkingMode::Enabled,
            ThinkingMode::Enabled => ThinkingMode::Adaptive,
        };
    }

    /// Advance the effort selector one step:
    /// Low → Medium → High → Max → Low.
    pub fn cycle_effort_level(&mut self) {
        self.effort_level = match self.effort_level {
            EffortLevel::Low => EffortLevel::Medium,
            EffortLevel::Medium => EffortLevel::High,
            EffortLevel::High => EffortLevel::Max,
            EffortLevel::Max => EffortLevel::Low,
        };
    }

    /// Advance the Agent Team size selector one step: 1x → 2x → … → 6x → 1x.
    pub fn cycle_agent_team_size(&mut self) {
        self.agent_team_size = if (1..6).contains(&self.agent_team_size) {
            self.agent_team_size + 1
        } else {
            1
        };
    }

    /// Stage a file for the next turn. Rejected (returns `false`) when
    /// the per-turn attachment cap is already reached or the file
    /// exceeds [`MAX_ATTACHMENT_BYTES`].
    pub fn add_attachment(&mut self, attachment: ChatAttachment) -> bool {
        if self.pending_attachments.len() >= MAX_ATTACHMENTS {
            return false;
        }
        if attachment.data.len() > MAX_ATTACHMENT_BYTES {
            return false;
        }
        self.pending_attachments.push(attachment);
        true
    }

    /// Drop the staged attachment at `index`; out-of-range is a no-op.
    pub fn remove_attachment(&mut self, index: usize) {
        if index < self.pending_attachments.len() {
            self.pending_attachments.remove(index);
        }
    }
}

package tech.zseven.openpencil

import android.os.SystemClock
import android.view.MotionEvent

/** Long-press delay (ms) before a right-click context menu. */
internal const val LONG_PRESS_MS = 500L
/** Movement (logical dp) that cancels a long-press candidate. */
internal const val LONG_PRESS_SLOP = 8f
/** Double-tap window (ms). */
internal const val DOUBLE_TAP_TIMEOUT_MS = 300L
/** Double-tap distance threshold (logical dp). */
internal const val DOUBLE_TAP_RADIUS_DP = 20f

/** Hit-test encoding (must match op-engine-ffi/src/editor_geometry.rs). */
internal const val HIT_EMPTY = 0
internal const val HIT_ANCHOR_BASE = 1 shl 24
internal const val HIT_HANDLE_IN_BASE = 2 shl 24
internal const val HIT_HANDLE_OUT_BASE = 3 shl 24
internal const val HIT_SEGMENT_BASE = 4 shl 24

/**
 * The platform cancel clock for the editor ABI: `MotionEvent.eventTime`
 * is NOT trustworthy on ACTION_CANCEL / synthetic cancels the shell
 * fabricates itself (two-finger takeover, long-press paste, geometry
 * transitions), so every synthetic Cancel uses the same monotonic
 * boot-uptime clock [SystemClock.uptimeMillis] — the same domain as
 * MotionEvent.eventTime, so the engine's global clock never sees a
 * bridge between two time domains.
 */
internal fun uptimeClockMs(): Long = SystemClock.uptimeMillis()

/**
 * Editor-mode touch state machine, split out of [OpSurfaceView] verbatim:
 * press/move/release streaming for the primary pointer, long-press
 * arming/firing (right-click or paste menu), and the two-finger pan +
 * pinch takeover. All engine calls mirror `OpSurfaceView.editorTouch`
 * exactly; gesture interpretation itself lives in the engine.
 *
 * Double-tap detection is also handled here: a double-tap within the
 * [DOUBLE_TAP_TIMEOUT_MS] window and [DOUBLE_TAP_RADIUS_DP] of the
 * first tap either enters geometry/vertex edit mode (for path nodes)
 * or exits it when already active.
 */
internal class OpSurfaceViewEditorTouch(private val view: OpSurfaceView) {

    private var primaryPointerId = -1
    private var longPressArmed = false
    private var longPressFired = false
    private val longPressRunnable = Runnable { fireLongPress() }
    private var lastMidX = 0f
    private var lastMidY = 0f
    private var lastPinchDist = 0f
    private var twoFingerActive = false
    private var editorReleaseSuppressed = false
    private var lastKnownX = 0f
    private var lastKnownY = 0f
    private var downX = 0f
    private var downY = 0f
    /** Timestamp of the most recent single tap (for double-tap detection). */
    private var lastTapTime = 0L
    /** Screen coordinates of the most recent single tap. */
    private var lastTapX = 0f
    private var lastTapY = 0f
    /** Whether geometry edit mode is currently active (mirrors the engine state). */
    private var geometryModeActive = false

    /** Active geometry drag state. */
    private var geometryDragActive = false
    /** Type of geometry drag: 0=none, 1=anchor, 2=handle_in, 3=handle_out. */
    private var geometryDragType = 0
    /** Anchor index being dragged. */
    private var geometryDragAnchorIdx = 0
    /** Node ID of the path being geometry-edited. */
    private var geometryDragNodeId = ""

    fun editorTouch(event: MotionEvent): Boolean {
        val inputDensity = view.committedInputDensity
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                primaryPointerId = event.getPointerId(0)
                longPressArmed = true
                longPressFired = false
                lastKnownX = event.x
                lastKnownY = event.y
                downX = event.x / inputDensity
                downY = event.y / inputDensity
                lastTapTime = event.eventTime
                lastTapX = event.x
                lastTapY = event.y

                // Geometry Edit Mode: if active, hit-test to start anchor/handle drag.
                if (geometryModeActive) {
                    val engine = view.editorEngine()
                    if (engine != 0L) {
                        val screenX = event.x / inputDensity
                        val screenY = event.y / inputDensity
                        val canvasW = view.width.toFloat() / inputDensity
                        val canvasH = view.height.toFloat() / inputDensity
                        val hit = OpNative.nativeEditorGeometryHitTest(
                            engine, screenX, screenY, canvasW.toInt(), canvasH.toInt()
                        )
                        if (hit != HIT_EMPTY) {
                            val (dragType, anchorIdx) = decodeHit(hit)
                            if (dragType != 0) {
                                // Get the node ID being edited.
                                val nodeId = OpNative.nativeEditorGeometryGetNodeId(engine)
                                if (nodeId.isNotEmpty()) {
                                    geometryDragNodeId = nodeId
                                    geometryDragAnchorIdx = anchorIdx
                                    geometryDragType = dragType
                                    geometryDragActive = true
                                    when (dragType) {
                                        1 -> OpNative.nativeEditorGeometryBeginAnchorDrag(
                                            engine, nodeId, anchorIdx, screenX, screenY
                                        )
                                        2 -> OpNative.nativeEditorGeometryBeginHandleDrag(
                                            engine, nodeId, anchorIdx, 0, screenX, screenY
                                        )
                                        3 -> OpNative.nativeEditorGeometryBeginHandleDrag(
                                            engine, nodeId, anchorIdx, 1, screenX, screenY
                                        )
                                    }
                                    view.requestFrame()
                                }
                            }
                        } else {
                            // Tap on empty space while in geometry mode: exit.
                            OpNative.nativeEditorGeometryExit(engine)
                            geometryModeActive = false
                            view.requestFrame()
                        }
                    }
                    return true
                }

                view.postDelayed(longPressRunnable, LONG_PRESS_MS)
                OpNative.nativeEditorPressAt(
                    view.editorEngine(),
                    event.x / inputDensity,
                    event.y / inputDensity,
                    event.eventTime,
                )
                view.settleEditorPressFlow()
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.pointerCount == 2) {
                    // Two fingers: pan + pinch take over.
                    longPressArmed = false
                    view.removeCallbacks(longPressRunnable)
                    // The first pointer already entered the editor press
                    // ladder. Cancel that capture before multi-touch starts
                    // so no marquee/node drag survives the takeover.
                    OpNative.nativeEditorCancelGestureAt(
                        view.editorEngine(),
                        uptimeClockMs(),
                    )
                    editorReleaseSuppressed = true
                    twoFingerActive = true
                    val (midX, midY) = midpoint(event)
                    lastMidX = midX
                    lastMidY = midY
                    lastPinchDist = distance(event)
                    OpNative.nativeEditorBeginTransform(
                        view.editorEngine(),
                        midX / inputDensity,
                        midY / inputDensity,
                    )
                }
            }
            MotionEvent.ACTION_MOVE -> {
                if (twoFingerActive && event.pointerCount >= 2) {
                    val (midX, midY) = midpoint(event)
                    val dx = (midX - lastMidX) / inputDensity
                    val dy = (midY - lastMidY) / inputDensity
                    val dist = distance(event)
                    val pinchDelta = PinchZoomDelta.wheelDelta(
                        previousDistance = lastPinchDist,
                        currentDistance = dist,
                    )
                    lastMidX = midX
                    lastMidY = midY
                    lastPinchDist = dist
                    if (dx != 0f || dy != 0f) {
                        OpNative.nativeEditorPan(
                            view.editorEngine(),
                            midX / inputDensity,
                            midY / inputDensity,
                            dx,
                            dy,
                        )
                    }
                    if (pinchDelta != 0f) {
                        OpNative.nativeEditorPinch(
                            view.editorEngine(),
                            midX / inputDensity,
                            midY / inputDensity,
                            pinchDelta,
                        )
                    }
                    view.requestFrame()
                } else if (primaryPointerId >= 0) {
                    // Geometry Edit Mode drag
                    if (geometryDragActive) {
                        val engine = view.editorEngine()
                        if (engine != 0L) {
                            val index = event.findPointerIndex(primaryPointerId)
                            if (index >= 0) {
                                val x = event.getX(index) / inputDensity
                                val y = event.getY(index) / inputDensity
                                val dx = x - downX
                                val dy = y - downY
                                downX = x
                                downY = y
                                when (geometryDragType) {
                                    1 -> OpNative.nativeEditorGeometryMoveAnchorDrag(
                                        engine, geometryDragNodeId, geometryDragAnchorIdx, dx, dy
                                    )
                                    2 -> OpNative.nativeEditorGeometryMoveHandleDrag(
                                        engine, geometryDragNodeId, geometryDragAnchorIdx, 0, dx, dy
                                    )
                                    3 -> OpNative.nativeEditorGeometryMoveHandleDrag(
                                        engine, geometryDragNodeId, geometryDragAnchorIdx, 1, dx, dy
                                    )
                                }
                                view.requestFrame()
                            }
                        }
                        return true
                    }
                    val index = event.findPointerIndex(primaryPointerId)
                    if (index >= 0) {
                        val x = event.getX(index) / inputDensity
                        val y = event.getY(index) / inputDensity
                        lastKnownX = event.getX(index)
                        lastKnownY = event.getY(index)
                        OpNative.nativeEditorMoveAt(view.editorEngine(), x, y, event.eventTime)
                        // Movement cancels the long-press candidate.
                        if (longPressArmed) {
                            val deltaX = x - downX
                            val deltaY = y - downY
                            if (deltaX * deltaX + deltaY * deltaY >
                                LONG_PRESS_SLOP * LONG_PRESS_SLOP
                            ) {
                                longPressArmed = false
                                view.removeCallbacks(longPressRunnable)
                            }
                        }
                        view.requestFrame()
                    }
                }
            }
            MotionEvent.ACTION_POINTER_UP -> {
                if (twoFingerActive) {
                    // End transform ownership before the remaining pointer is
                    // re-armed; its eventual Up must never release the press
                    // ladder cancelled at second-finger Down.
                    OpNative.nativeEditorCancelGestureAt(
                        view.editorEngine(),
                        uptimeClockMs(),
                    )
                    twoFingerActive = false
                    // Track the remaining physical pointer only so its final
                    // Up can terminate this suppressed stream. A fresh Down
                    // is required before press/move/release may resume.
                    val index = if (event.actionIndex == 0) 1 else 0
                    if (index < event.pointerCount) {
                        primaryPointerId = event.getPointerId(index)
                        longPressArmed = false
                    }
                }
            }
            MotionEvent.ACTION_UP -> {
                view.removeCallbacks(longPressRunnable)
                if (twoFingerActive) {
                    OpNative.nativeEditorCancelGestureAt(
                        view.editorEngine(),
                        uptimeClockMs(),
                    )
                    twoFingerActive = false
                } else if (geometryDragActive) {
                    // End geometry drag
                    val engine = view.editorEngine()
                    if (engine != 0L) {
                        when (geometryDragType) {
                            1 -> OpNative.nativeEditorGeometryEndAnchorDrag(engine)
                            2, 3 -> OpNative.nativeEditorGeometryEndHandleDrag(engine)
                        }
                    }
                    geometryDragActive = false
                    geometryDragType = 0
                    geometryDragAnchorIdx = 0
                    geometryDragNodeId = ""
                    view.requestFrame()
                } else if (!longPressFired && !editorReleaseSuppressed) {
                    // Double-tap detection: check if this up follows
                    // a down within DOUBLE_TAP_TIMEOUT_MS and
                    // DOUBLE_TAP_RADIUS_DP of the previous tap.
                    if (isDoubleTap(event)) {
                        handleDoubleTap(event, inputDensity)
                    } else {
                        val x = event.x / inputDensity
                        val y = event.y / inputDensity
                        OpNative.nativeEditorReleaseAt(view.editorEngine(), x, y, event.eventTime)
                    }
                }
                resetTracking()
                view.requestFrame()
            }
            MotionEvent.ACTION_CANCEL -> {
                // A platform cancellation must never run the release ladder:
                // release may commit a deferred tap, drag/drop, or history.
                // ACTION_CANCEL is a real MotionEvent, so it carries its own
                // eventTime; only the shell-fabricated cancels below fall back
                // to SystemClock.uptimeMillis().
                val engine = view.editorEngine()
                if (geometryDragActive && engine != 0L) {
                    when (geometryDragType) {
                        1 -> OpNative.nativeEditorGeometryEndAnchorDrag(engine)
                        2, 3 -> OpNative.nativeEditorGeometryEndHandleDrag(engine)
                    }
                    geometryDragActive = false
                    geometryDragType = 0
                    geometryDragAnchorIdx = 0
                    geometryDragNodeId = ""
                }
                OpNative.nativeEditorCancelGestureAt(engine, event.eventTime)
                resetTracking()
                view.requestFrame()
            }
            else -> return false
        }
        return true
    }

    fun resetTracking() {
        view.removeCallbacks(longPressRunnable)
        primaryPointerId = -1
        longPressArmed = false
        longPressFired = false
        twoFingerActive = false
        editorReleaseSuppressed = false
        geometryDragActive = false
        geometryDragType = 0
        geometryDragAnchorIdx = 0
        geometryDragNodeId = ""
        lastMidX = 0f
        lastMidY = 0f
        lastPinchDist = 0f
        lastKnownX = 0f
        lastKnownY = 0f
        downX = 0f
        downY = 0f
        lastTapTime = 0L
        lastTapX = 0f
        lastTapY = 0f
    }

    /** Returns true if this event constitutes a double-tap. */
    private fun isDoubleTap(event: MotionEvent): Boolean {
        val now = event.eventTime
        if (now - lastTapTime > DOUBLE_TAP_TIMEOUT_MS) return false
        val dx = event.x - lastTapX
        val dy = event.y - lastTapY
        return dx * dx + dy * dy <= DOUBLE_TAP_RADIUS_DP * DOUBLE_TAP_RADIUS_DP
    }

    /** Handle a double-tap: enter or exit geometry edit mode. */
    private fun handleDoubleTap(event: MotionEvent, density: Float) {
        val engine = view.editorEngine()
        if (engine == 0L) return
        if (geometryModeActive) {
            // Double-tap while in geometry mode: exit.
            OpNative.nativeEditorGeometryExit(engine)
            geometryModeActive = false
        } else {
            // Double-tap: hit-test and enter geometry mode
            // for the node under the finger if applicable.
            val screenX = event.x / density
            val screenY = event.y / density
            val canvasW = view.width.toFloat() / density
            val canvasH = view.height.toFloat() / density
            val hit = OpNative.nativeEditorGeometryHitTest(
                engine, screenX, screenY, canvasW.toInt(), canvasH.toInt(),
            )
            if (hit != HIT_EMPTY) {
                // Hit on a path node: get the selected node ID and enter geometry mode.
                // The engine's selection should contain the path node that was double-tapped.
                val nodeId = OpNative.nativeEditorGeometryGetNodeId(engine)
                // If no node is currently being edited, we need to find the node under the finger.
                // For now, use the current selection. The hit-test result tells us we hit something.
                val targetNodeId = if (nodeId.isEmpty()) {
                    // Fallback: use empty string to let engine use current selection.
                    ""
                } else {
                    nodeId
                }
                OpNative.nativeEditorGeometryEnter(engine, targetNodeId)
                geometryModeActive = true
            }
        }
        view.requestFrame()
    }

    /** Decode hit-test result: returns (dragType, anchorIdx). */
    private fun decodeHit(hit: Int): Pair<Int, Int> {
        val hitType = hit ushr 24
        val idx = hit and 0xFFFFFF
        return when (hitType) {
            1 -> 1 to idx          // anchor
            2 -> 2 to idx          // handle_in
            3 -> 3 to idx          // handle_out
            else -> 0 to 0         // segment or unknown = no drag
        }
    }

    /** Drops only the pending long-press timer (teardown path). */
    fun detach() {
        view.removeCallbacks(longPressRunnable)
    }

    private fun fireLongPress() {
        longPressArmed = false
        if (!view.isFrameGateOpen || view.editorEngine() == 0L) return
        longPressFired = true
        val inputDensity = view.committedInputDensity
        // The Down at press time already ran the engine's press ladder, so
        // the engine's IME focus reflects THIS touch: focused means the
        // finger is holding an editable text field — offer Paste instead of
        // the right-click context menu.
        if (view.showPasteMenuIfEditingText(lastKnownX, lastKnownY)) {
            // The press capture opened at Down must not leak while the
            // release is suppressed by longPressFired.
            OpNative.nativeEditorCancelGestureAt(view.editorEngine(), uptimeClockMs())
        } else {
            OpNative.nativeEditorRightPress(
                view.editorEngine(),
                lastKnownX / inputDensity,
                lastKnownY / inputDensity,
            )
        }
        view.requestFrame()
    }

    private fun midpoint(event: MotionEvent): Pair<Float, Float> {
        var sx = 0f
        var sy = 0f
        for (i in 0 until event.pointerCount) {
            sx += event.getX(i)
            sy += event.getY(i)
        }
        lastKnownX = sx / event.pointerCount
        lastKnownY = sy / event.pointerCount
        return lastKnownX to lastKnownY
    }

    private fun distance(event: MotionEvent): Float {
        if (event.pointerCount < 2) return 0f
        val dx = event.getX(0) - event.getX(1)
        val dy = event.getY(0) - event.getY(1)
        return Math.sqrt((dx * dx + dy * dy).toDouble()).toFloat()
    }
}

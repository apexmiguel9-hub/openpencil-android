//! In-app Git panel state — commit / diff / merge / clone data plus
//! the `GitPanelState` snapshot the desktop host fills from its
//! `GitSession`. All plain data, so `op-editor-core` stays wasm32-clean.
//!
//! Split out of the `editor_ui_state` spine (800-line file ceiling);
//! every type is re-exported from there, so import paths are unchanged.

/// One commit row shown in the Git panel — plain data snapshotted
/// by the desktop host from its git session. The platform-free
/// widget layer only paints it; it never calls git itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitSummary {
    /// Abbreviated commit hash.
    pub short_hash: String,
    /// First line of the commit message.
    pub summary: String,
    /// Author display name.
    pub author: String,
    /// Pre-formatted relative-time label (`now` / `5m` / `2h` / …),
    /// computed host-side against the wall clock when the snapshot is
    /// taken (TS `formatCompactTime`). The widget layer is platform-free
    /// and has no wall clock, so it cannot derive this itself.
    pub time_label: String,
    /// `true` for the root commit (no parent). The expanded detail card
    /// shows the "initial commit — nothing to diff" line for it (TS
    /// `git.history.diff.initialCommit`).
    pub is_initial: bool,
}

/// One `.op` candidate in the tracked-file picker (TS `GitCandidateFileInfo`).
/// Plain data the host enumerates from the repo; the widget only paints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCandidateFile {
    /// Absolute path — the bind argument.
    pub path: String,
    /// Repo-relative path — the row title.
    pub relative_path: String,
    /// Number of commits that touched this file (the "N milestones" label).
    pub milestone_count: u32,
    /// Pre-formatted relative time of the last commit touching it, or empty.
    pub last_commit_time: String,
    /// First line of the last commit's message, if any.
    pub last_commit_message: Option<String>,
}

/// One node-level change in a commit's semantic diff (TS `NodePatch`,
/// rendered as `<op> <nodeId>` in the inline detail card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDiffPatch {
    /// `add` / `remove` / `modify` / `move`.
    pub op: String,
    /// The affected node's id.
    pub node_id: String,
}

/// Aggregated semantic diff of one commit against its parent — the TS
/// `engineDiff` result that drives `GitPanelHistoryDiff`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommitDiffSummary {
    /// Distinct parent ids touched by any patch.
    pub frames_changed: u32,
    pub nodes_added: u32,
    pub nodes_removed: u32,
    pub nodes_modified: u32,
    /// Per-node patch list (newest-first walk order).
    pub patches: Vec<CommitDiffPatch>,
}

/// Lazy state of the expanded commit's inline diff (TS `DiffState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitDiffView {
    /// The host is computing the diff on a worker / this frame.
    Loading,
    /// The root commit — no parent to diff against.
    Initial,
    /// Diff computed, but no node changed (rare; e.g. metadata-only).
    NoChanges,
    /// The diff could not be computed (parse / git error). Carries the message.
    Error(String),
    /// Computed diff ready to render.
    Ready(CommitDiffSummary),
}

/// One changed file in the Git panel's staging list — plain data
/// snapshotted by the desktop host from `git status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFileEntry {
    /// Repo-relative path.
    pub path: String,
    /// Whether the change is staged in the index.
    pub staged: bool,
    /// Single-char status: `M` / `A` / `D` / `R` / `?` / `U`.
    pub status: char,
}

/// What a Git-panel diff request should diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitDiffTarget {
    /// The whole working tree's unstaged changes (`git diff`).
    WorkingTree,
    /// One repo-relative path's working-tree changes (`git diff -- <path>`).
    Path(String),
    /// The full patch a commit introduced (`git show <rev>`).
    Commit(String),
}

/// A unified-diff view open inside the Git panel — filled by the
/// desktop host from a background `git diff` / `git show` job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitDiffView {
    /// Human label for the diff (a path, or a commit summary).
    pub title: String,
    /// The diff text split into lines for per-line colouring.
    pub lines: Vec<String>,
    /// Index of the first visible line — paged by the ▲ / ▼ buttons
    /// and the mouse wheel.
    pub scroll: usize,
    /// First visible character column — long lines scroll sideways
    /// with the ◀ / ▶ buttons.
    pub h_scroll: usize,
    /// Repo-relative path when this diff is a single working-tree
    /// file that supports per-hunk staging — `None` for a commit
    /// diff or the whole-tree diff.
    pub stage_path: Option<String>,
}

/// One node conflict in the interactive merge-resolution view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflictRow {
    /// The conflicting node's `id`.
    pub id: String,
    /// Display label — the node's name / type.
    pub label: String,
    /// Human label for the conflict kind (e.g. "both modified").
    pub kind: String,
    /// Whether "theirs" is a selectable resolution — `false` for a
    /// structural conflict, which can only be resolved to "ours".
    pub theirs_allowed: bool,
    /// The chosen side: `false` = keep ours, `true` = take theirs.
    pub take_theirs: bool,
}

/// One conflicted `.op` file in the merge-resolution view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResolveFile {
    /// Repo-relative path.
    pub path: String,
    /// The three merge-stage blobs — kept so "Apply" can re-run the
    /// structured merge with the user's per-node choices.
    pub base: String,
    pub ours: String,
    pub theirs: String,
    /// The file's node conflicts.
    pub conflicts: Vec<MergeConflictRow>,
}

/// Interactive merge-conflict-resolution state — set when a branch
/// merge conflicts entirely in structured `.op` files. The panel
/// shows each conflicting node with an ours/theirs choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeResolveState {
    /// The branch being merged in.
    pub branch: String,
    /// The conflicted `.op` files.
    pub files: Vec<MergeResolveFile>,
}

impl MergeResolveState {
    /// Total conflict count across every file.
    pub fn total(&self) -> usize {
        self.files.iter().map(|f| f.conflicts.len()).sum()
    }

    /// Every conflict row, flattened in file order — the order the
    /// panel paints and hit-tests.
    pub fn rows(&self) -> Vec<&MergeConflictRow> {
        self.files.iter().flat_map(|f| &f.conflicts).collect()
    }

    /// Set the choice of the flat-indexed conflict row. A `theirs`
    /// choice on a structural conflict falls back to "ours".
    pub fn set_choice(&mut self, flat_index: usize, take_theirs: bool) {
        let mut i = 0;
        for file in &mut self.files {
            for row in &mut file.conflicts {
                if i == flat_index {
                    row.take_theirs = take_theirs && row.theirs_allowed;
                    return;
                }
                i += 1;
            }
        }
    }
}

/// Which view the ready-state overflow `…` popover is showing. The
/// top-level menu opens subviews in place (mirrors the TS header's
/// `overflowView` state machine), resetting to `Menu` on close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitOverflowView {
    /// The top-level action list.
    #[default]
    Menu,
    /// The remote-settings subview — origin URL + HTTPS credential.
    RemoteSettings,
    /// The tracked-file picker subview — pick which `.op` the panel tracks.
    TrackedPicker,
    /// The SSH-keys subview — list keys + import / generate.
    SshKeys,
}

/// Which sub-mode the branch-picker dropdown is showing (mirrors the
/// TS `GitPanelBranchPicker` `mode` state machine). Resets to `List`
/// when the dropdown closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitBranchPickerMode {
    /// Branch list + the `新建分支` / `合并分支` footer actions.
    #[default]
    List,
    /// Inline `新建分支` form — a branch-name text input.
    Create,
    /// `合并分支` mode — pick a non-current branch to merge into HEAD.
    Merge,
}

/// An interactive action requested from the Git panel. The desktop
/// host drains it from [`GitPanelState::pending_action`] and runs it
/// against its `GitSession` (the widget layer never calls git).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitPanelAction {
    /// Empty-state Init card — create a local repo for the saved doc.
    InitRepo,
    /// Empty-state Open card — pick + bind an existing repo folder.
    OpenRepo,
    /// Empty-state Clone card — clone a remote into a chosen folder.
    CloneRepo,
    /// Re-read repository state into the panel.
    Refresh,
    /// Pull the current branch's upstream.
    Pull,
    /// Push the current branch to its upstream.
    Push,
    /// Stage + commit the tracked document with the panel's
    /// `commit_input`.
    Commit,
    /// Ready-view "Save milestone": save the current design to the
    /// tracked `.op`, stage it, and commit with the panel's
    /// `commit_input` — the TS `commitMilestone` flow. Unlike
    /// [`GitPanelAction::Commit`] (which commits a pre-assembled staged
    /// index) this snapshots the live editor state in one click.
    CommitMilestone,
    /// Switch the working tree to the named branch.
    SwitchBranch(String),
    /// Create a new branch with the given name (from the inline
    /// `新建分支` form) and switch to it.
    CreateBranch(String),
    /// Add / re-point the `origin` remote to the given URL.
    SetRemote(String),
    /// Generate (or reuse) an SSH key for the `origin` host and bind
    /// it as that host's stored credential.
    SetupSshAuth,
    /// Store an HTTPS credential for the `origin` host — the payload
    /// is the `username:token` text typed into the Remotes section.
    SetHttpsAuth(String),
    /// Merge the named branch into the current one through an
    /// isolated worktree (the live tree is never marked up).
    MergeBranch(String),
    /// Abort an in-progress merge, restoring the pre-merge state.
    AbortMerge,
    /// Finalize an in-progress merge once its conflicts are resolved.
    CompleteMerge,
    /// Compute a unified diff and open it in the panel's diff view.
    ShowDiff(GitDiffTarget),
    /// Toggle whether the named changed file is staged in the index.
    ToggleStageFile(String),
    /// Stage a single hunk of the open diff — `(path, hunk_index)`.
    StageHunk(String, usize),
    /// Re-run the branch merge applying the per-node ours/theirs
    /// choices the user picked in the merge-resolution view.
    ApplyMergeResolution,
    /// Clone-form "选择…" — open a native folder picker and write the
    /// chosen path into the form's `dest` field.
    PickCloneDest,
    /// Clone-form submit — `git clone <url> <dest>` on a worker thread,
    /// then bind the cloned repo. Reads url / dest from `clone_form`.
    SubmitClone,
    /// Roll the tracked document back to the given commit (hash) and
    /// reload the editor — the TS `restoreCommit`. The payload is the
    /// commit's (short) hash from the expanded detail card.
    RestoreCommit(String),
    /// Copy the given commit hash to the OS clipboard (TS copy-hash).
    CopyHash(String),
    /// Compute the semantic diff of `recent_commits[index]` against its
    /// parent and store it in `expanded_commit_diff` (TS `computeDiff`,
    /// triggered when a commit row's detail card is expanded).
    LoadCommitDiff(usize),
    /// Overflow "切换跟踪文件" — enumerate the repo's `.op` candidates into
    /// `candidate_files` and open the tracked-file picker subview.
    EnterTrackedPicker,
    /// Bind the panel to the given `.op` path (TS `bindTrackedFile`). The
    /// `bool` is "also load it into the editor" (TS "track and open").
    BindTrackedFile(String, bool),
    /// Overflow "清除提交作者" — clear the stored commit-author identity.
    ClearAuthor,
    /// Overflow "关闭仓库" — unbind the repository and reset to empty state.
    CloseRepo,
    /// Overflow "SSH 密钥" — enumerate stored SSH keys + open the subview.
    EnterSshKeys,
    /// SSH subview "导入现有密钥" — pick a private key file and import it.
    ImportSshKey,
    /// Remote-settings "获取" — run `git fetch` on the origin remote.
    FetchRemote,
    /// Commit-signature form "保存" — write the name/email drafts into the
    /// repo identity, then re-fire the pending milestone commit.
    SaveAuthor,
}

/// Which clone-form text field has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneField {
    /// The remote URL (`https://…` / `git@…`).
    Url,
    /// The local destination folder.
    Dest,
}

/// Inline clone-wizard state — `Some` on `GitPanelState.clone_form`
/// puts the panel into the clone view (a port of the TS
/// `GitPanelCloneForm`). Reached from the empty-state Clone card. Plain
/// data so the widget layer stays wasm-clean; the desktop host owns the
/// folder picker + the `git clone` job.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloneFormState {
    /// Remote URL draft.
    pub url_input: jian_core::text_input::TextInputState,
    /// Local destination-folder draft.
    pub dest_input: jian_core::text_input::TextInputState,
    /// Which field has keyboard focus (`None` = no caret).
    pub focus: Option<CloneField>,
    /// `true` while the `git clone` worker runs — disables the form.
    pub cloning: bool,
    /// Last clone error (validation or a failed `git clone`), shown
    /// under the fields.
    pub error: Option<String>,
}

/// Git panel state — a plain-data snapshot the desktop host fills
/// from its `GitSession`. The widget layer reads it to paint the
/// floating Git panel; it carries no git handles, so it stays
/// wasm-clean. Refreshed whenever the panel is opened.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitPanelState {
    /// Whether the floating Git panel is currently shown.
    pub open: bool,
    /// Whether the open document lives inside a git repository.
    pub in_repo: bool,
    /// Whether the open document has an on-disk path — gates the
    /// empty-state "Init" card (can't create local history for an
    /// unsaved doc). Set by the host on each panel refresh.
    pub has_saved_file: bool,
    /// Which empty-state onboarding card the cursor is over, if any
    /// (`0` = Init, `1` = Open, `2` = Clone). Drives the per-card hover
    /// effect and the disabled-Init hint pill (shown only while card
    /// `0` is hovered with no saved file). Updated by the host on
    /// cursor-move.
    pub empty_hovered_card: Option<u8>,
    /// Ready-state header: whether the cursor is over the `⎇ <branch> ▾`
    /// button. Drives its `hover:bg-accent` wash. Updated by the host on
    /// cursor-move.
    pub branch_button_hovered: bool,
    /// Which plain Git action button (pull / push / overflow / commit /
    /// milestone / refresh) the cursor is over — drives its
    /// `theme.button_hover` wash. Updated by the host on cursor-move.
    pub button_hover: Option<crate::git_button_state::GitButton>,
    /// Ready-state header: whether the `…` overflow popover is open
    /// (switch-tracked / clear-author / remote-settings › / SSH-keys › /
    /// close-repo). Mirrors the TS header's local `overflowOpen`.
    pub overflow_open: bool,
    /// Shared interaction state for the top-level overflow menu rows.
    pub overflow_menu: jian_widgets::components::menu::MenuState,
    /// Which view the overflow popover is showing — the top-level menu
    /// or one of its subviews (remote settings). Resets to `Menu` each
    /// time the popover closes. Mirrors the TS header's `overflowView`.
    pub overflow_view: GitOverflowView,
    /// Ready-state header: whether the branch-picker dropdown (opened
    /// from the `⎇ <branch> ▾` button) is open.
    pub branch_picker_open: bool,
    /// Shared interaction state for branch-picker dropdown rows.
    pub branch_picker_menu: jian_widgets::components::menu::MenuState,
    /// Current branch name of that repository.
    pub branch: Option<String>,
    /// All local branch names, sorted — the panel lists them for
    /// one-click switching.
    pub branches: Vec<String>,
    /// Which branch-picker sub-mode is showing (list / create / merge).
    pub branch_picker_mode: GitBranchPickerMode,
    /// Draft branch name typed into the inline `新建分支` form.
    pub branch_create_input: jian_core::text_input::TextInputState,
    /// Whether the `新建分支` name input holds keyboard focus.
    pub branch_create_focused: bool,
    /// Number of changed (dirty) files in the working tree.
    pub dirty_count: usize,
    /// Commits the current branch is ahead of its upstream — gates the
    /// Push button (TS disables Push when `ahead === 0`).
    pub ahead: u32,
    /// Commits the local branch is behind its upstream (remote-settings row).
    pub behind: u32,
    /// The `origin` remote's host (e.g. `github.com`), parsed host-side.
    /// Drives the remote-settings credentials row; `None` = no host detected.
    pub remote_host: Option<String>,
    /// Stored-credential kind for `remote_host`: `"token"` / `"ssh"` /
    /// `"none"` (empty when there's no host). Host-filled.
    pub stored_auth: String,
    /// Number of files with unresolved merge conflicts.
    pub conflicted_count: usize,
    /// Whether a merge is in progress — drives the panel's conflict
    /// mode (conflicted-file list + Abort / Complete actions).
    pub merging: bool,
    /// Repo-relative paths with unresolved merge conflicts.
    pub conflicted_files: Vec<String>,
    /// Changed files in the working tree — the per-file staging list.
    pub changed_files: Vec<GitFileEntry>,
    /// Configured remotes as display strings — `name → url`.
    pub remotes: Vec<String>,
    /// Draft URL typed into the Remotes section's input box.
    pub remote_input: jian_core::text_input::TextInputState,
    /// Whether the remote-URL input holds keyboard focus.
    pub remote_focused: bool,
    /// Draft `username:token` typed into the HTTPS-credential input.
    pub https_input: jian_core::text_input::TextInputState,
    /// Whether the HTTPS-credential input holds keyboard focus.
    pub https_focused: bool,
    /// Most-recent commits, newest first.
    pub recent_commits: Vec<GitCommitSummary>,
    /// Index into `recent_commits` of the row whose inline detail card
    /// (里程碑详情 — restore + copy-hash) is expanded, if any. Pure UI
    /// state, toggled by clicking a commit row. Cleared host-side when
    /// the commit list changes so it can't point at a stale commit
    /// (TS keys the card by hash; the widget layer keys by index).
    pub expanded_commit: Option<usize>,
    /// Lazy semantic diff for the expanded commit (TS `GitPanelHistoryDiff`).
    /// `None` when no card is open; otherwise loading / initial / ready /
    /// error. The host fills it after a `LoadCommitDiff` action.
    pub expanded_commit_diff: Option<CommitDiffView>,
    /// Candidate `.op` files for the tracked-file picker subview, host-filled
    /// when the picker opens (TS `RepoMeta.candidateFiles`).
    pub candidate_files: Vec<GitCandidateFile>,
    /// The picker's currently-selected candidate index, if any.
    pub tracked_picker_selected: Option<usize>,
    /// Shared select state for the tracked-file picker row list.
    pub tracked_picker: jian_widgets::components::select::SelectState,
    /// SSH key names for the SSH-keys subview (host-filled on open).
    pub ssh_keys: Vec<String>,
    /// Commit-message draft typed into the panel's input box.
    pub commit_input: jian_core::text_input::TextInputState,
    /// Whether the commit-message input holds keyboard focus.
    pub commit_focused: bool,
    /// Set when a milestone "save" was skipped because the saved design
    /// matched the last commit — the ready view shows a "未检测到变更" hint
    /// under the commit box. Cleared when the user re-engages the input.
    pub commit_no_changes: bool,
    /// Whether the commit-signature form (`提交署名`) is showing in place of
    /// the commit box — raised when a commit is attempted with no committer
    /// identity (TS `authorPromptVisible`). The pending message stays in
    /// `commit_input` and the commit re-fires after a successful save.
    pub author_prompt: bool,
    /// Name / email drafts typed into the commit-signature form.
    pub author_name_input: jian_core::text_input::TextInputState,
    pub author_email_input: jian_core::text_input::TextInputState,
    /// Which signature-form field holds keyboard focus.
    pub author_name_focused: bool,
    pub author_email_focused: bool,
    /// Interactive action requested by a panel click / Enter —
    /// drained and executed by the desktop host.
    pub pending_action: Option<GitPanelAction>,
    /// Whether a background `git pull` is currently in flight — the
    /// panel shows a "Pulling…" status and disables the Pull button.
    pub pulling: bool,
    /// Whether a background `git push` is currently in flight — the
    /// panel shows a "Pushing…" status and disables the Push button.
    pub pushing: bool,
    /// Whether the panel is awaiting its first repository snapshot
    /// after opening / a repo switch. While `true` the panel shows a
    /// "Loading…" state instead of the (possibly stale) prior data.
    pub loading: bool,
    /// Open diff view — `Some` puts the panel into diff mode, showing
    /// a scrollable unified diff instead of the status / action area.
    /// Closed by the diff view's ✕ button.
    pub diff: Option<GitDiffView>,
    /// Interactive merge-conflict-resolution view — `Some` puts the
    /// panel into resolution mode, listing each conflicting node with
    /// an ours/theirs choice. Cleared on Apply / Cancel.
    pub merge_resolve: Option<MergeResolveState>,
    /// Inline clone wizard — `Some` puts the panel into the clone view
    /// (URL + destination + Clone / Cancel), reached from the empty-state
    /// Clone card. Cleared on Cancel or a successful clone.
    pub clone_form: Option<CloneFormState>,
}

impl GitPanelState {
    /// Whether the ready-view header popovers (the branch picker and the
    /// `…` overflow menu) may be open in this state. They live only in
    /// the bound, non-merging ready view. A dirty working tree still
    /// shows that view (TS parity — the ready view no longer gates on a
    /// clean tree), so dirtiness does NOT disqualify them; only an
    /// unbound repo or an in-progress merge does. A background status
    /// refresh that lands a non-ready state uses this to force-close the
    /// popovers so they can't go stale and dead-end input.
    pub fn header_popovers_allowed(&self) -> bool {
        self.in_repo && !self.merging
    }

    pub fn defocus_commit_input(&mut self, now_ms: u64) -> bool {
        let was_focused = self.commit_focused;
        self.commit_focused = false;
        let commit_caret = self.commit_input.caret();
        self.commit_input.set_caret(commit_caret, now_ms);
        was_focused
    }

    /// Drop keyboard focus from every git-panel text input — commit
    /// message, remote URL, HTTPS credential, branch-create name, the
    /// author signature pair, and the clone form's URL / destination.
    /// Drafts persist (focus flags only) so re-engaging an input shows
    /// what was typed. Returns `true` when any input was focused so
    /// blank-press / panel-close callers know to repaint.
    pub fn defocus_text_inputs(&mut self) -> bool {
        let clone_focused = self
            .clone_form
            .as_mut()
            .map(|form| {
                let url_caret = form.url_input.caret();
                form.url_input.set_caret(url_caret, 0);
                let dest_caret = form.dest_input.caret();
                form.dest_input.set_caret(dest_caret, 0);
                form.focus.take().is_some()
            })
            .unwrap_or(false);
        let commit_focused = self.defocus_commit_input(0);
        let was_focused = clone_focused
            || commit_focused
            || self.remote_focused
            || self.https_focused
            || self.branch_create_focused
            || self.author_name_focused
            || self.author_email_focused;
        self.remote_focused = false;
        self.https_focused = false;
        self.branch_create_focused = false;
        self.author_name_focused = false;
        self.author_email_focused = false;
        let remote_caret = self.remote_input.caret();
        self.remote_input.set_caret(remote_caret, 0);
        let https_caret = self.https_input.caret();
        self.https_input.set_caret(https_caret, 0);
        let branch_caret = self.branch_create_input.caret();
        self.branch_create_input.set_caret(branch_caret, 0);
        let author_name_caret = self.author_name_input.caret();
        self.author_name_input.set_caret(author_name_caret, 0);
        let author_email_caret = self.author_email_input.caret();
        self.author_email_input.set_caret(author_email_caret, 0);
        was_focused
    }

    /// Open the tracked-file picker row list and reset transient select
    /// interaction state for a fresh candidate set.
    pub fn open_tracked_picker(&mut self) {
        self.tracked_picker.open = true;
        self.tracked_picker.hover = None;
        self.tracked_picker.pressed = None;
        self.tracked_picker.scroll.offset = 0.0;
        self.tracked_picker_selected = None;
    }

    /// Close the tracked-file picker and clear its transient selection.
    pub fn close_tracked_picker(&mut self) -> bool {
        let changed = self.tracked_picker.open
            || self.tracked_picker.hover.is_some()
            || self.tracked_picker.pressed.is_some()
            || self.tracked_picker.scroll.offset != 0.0
            || self.tracked_picker_selected.is_some();
        self.tracked_picker.open = false;
        self.tracked_picker.hover = None;
        self.tracked_picker.pressed = None;
        self.tracked_picker.scroll.offset = 0.0;
        self.tracked_picker_selected = None;
        changed
    }
}

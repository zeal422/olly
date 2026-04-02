#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    Models,
    Workspaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelBucket {
    Cloud,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    SectionHeader(SidebarSection),
    ModelGroupHeader(ModelBucket),
    Model(usize),
    WorkspaceCreate,
    Workspace(usize),
    WorkspaceSessionsLabel(usize),
    WorkspaceSession(usize, usize),
}

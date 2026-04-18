#[derive(Clone, Debug)]
pub enum ConflictPart {
    Common(String),
    Conflict {
        ours: String,
        theirs: String,
        resolution: ConflictChoice,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ConflictChoice {
    #[default]
    Unresolved,
    Ours,
    Theirs,
    Both,
}

#[derive(Clone, Debug)]
pub struct ConflictData {
    pub path: String,
    pub sections: Vec<ConflictPart>,
}

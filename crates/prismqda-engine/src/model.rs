//! Small value types shared by the engine and its callers.

/// Declares a fieldless enum that maps to and from the exact strings stored in the project file.
macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The text stored in the project file.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;
            fn from_str(s: &str) -> std::result::Result<Self, String> {
                match s {
                    $($text => Ok($name::$variant),)+
                    _ => Err(format!("'{}' is not one of: {}", s, [$($text),+].join(", "))),
                }
            }
        }
    };
}

str_enum! {
    /// Whether coding is free-span or tied to predefined units (specification 8.1).
    CodingMode { FreeSpan => "free_span", Unit => "unit" }
}

str_enum! {
    /// Human or model coder (specification 18.1).
    CoderRole { Human => "human", Model => "model" }
}

str_enum! {
    /// What kind of material a source is (specification section 5).
    SourceType {
        Document => "document",
        Pdf => "pdf",
        Survey => "survey",
        Transcript => "transcript",
        Social => "social",
        Other => "other",
    }
}

str_enum! {
    /// The analysis unit a source is split into (specification 6.3).
    UnitKind {
        Sentence => "sentence",
        Paragraph => "paragraph",
        SpeakerTurn => "speaker_turn",
        SurveyResponse => "survey_response",
        Post => "post",
        Comment => "comment",
    }
}

str_enum! {
    /// Where a coding came from. Accepted AI work keeps its origin permanently (specification 19.3).
    Origin {
        Human => "human",
        AiSuggestionAccepted => "ai_suggestion_accepted",
        AiApplied => "ai_applied",
    }
}

str_enum! {
    /// Level of measurement of an attribute (specification 7.3). The statistics module uses it to offer only valid tests.
    Level {
        Nominal => "nominal",
        Ordinal => "ordinal",
        Interval => "interval",
        Ratio => "ratio",
        Date => "date",
        Text => "text",
    }
}

str_enum! {
    /// What an attribute describes (specification 7.3).
    AppliesTo { Case => "case", Source => "source", Unit => "unit" }
}

str_enum! {
    /// A kind of formatting kept from an imported document (specification section 5).
    RunKind {
        Heading => "heading",
        Bold => "bold",
        Italic => "italic",
        ListItem => "list_item",
        TableCell => "table_cell",
        Page => "page",
    }
}

str_enum! {
    /// The kind of memo (specification 10.1).
    MemoKind {
        Analytic => "analytic",
        Reflexive => "reflexive",
        MethodDecision => "method_decision",
        Todo => "todo",
    }
}

str_enum! {
    /// What a memo is attached to (specification 10.1).
    MemoScope {
        Project => "project",
        Source => "source",
        Code => "code",
        Coding => "coding",
        Case => "case",
        Phase => "phase",
        Standalone => "standalone",
    }
}

str_enum! {
    /// Who made a suggestion that waits for review (specification 12.3 and 19.3).
    ProposalOrigin { Autocode => "autocode", Ai => "ai" }
}

impl SourceType {
    /// The default unit for each source type (specification 6.3).
    pub fn default_unit_kind(self) -> UnitKind {
        match self {
            SourceType::Document | SourceType::Pdf | SourceType::Other => UnitKind::Sentence,
            SourceType::Transcript => UnitKind::SpeakerTurn,
            SourceType::Survey => UnitKind::SurveyResponse,
            SourceType::Social => UnitKind::Comment,
        }
    }
}

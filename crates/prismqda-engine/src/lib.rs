//! The PrismQDA engine: a headless library that owns the project file.
//!
//! It is built and tested before any interface exists (specification section 3.2). The command-line
//! harness in `prismqda-cli` and, later, the desktop app both call this crate and nothing else
//! touches the project file.
//!
//! What is here: the project file with its published schema (section 4), crash-safe writes, dated
//! backups, read-only opening of newer files, migration of older ones, and the append-only event
//! log (sections 4.2 and 27). Coding, sources, codes, and coders have their first operations; the rest of
//! the entity tables exist in the schema and gain operations milestone by milestone.

mod ai;
mod autocode;
mod calendar;
mod case_table;
mod cases;
mod codebook;
mod concept_map;
mod crypto;
mod crosstab;
mod dataprep;
mod datatable;
mod deid;
mod deid_suggest;
mod edit;
mod error;
mod event;
mod explore;
mod adjudication;
mod image;
mod final_layer;
mod ids;
mod region_final;
mod merge;
mod packet;
mod teaching;
mod passage_links;
mod privacy;
mod theme_ai;
mod themes;
mod text_analytics;
pub mod import;
pub mod paths;
mod media;
mod memo;
mod model;
mod ops;
mod pdf;
mod project;
mod proposal;
mod query;
mod regions;
mod reliability;
mod integration;
mod mixed;
mod saved;
mod stat_notebook;
pub mod schema;
mod search;
mod segments;
mod stat_charts;
mod statistics;
mod template;
mod undo;
mod variables;

pub use error::{Error, Result};
pub use codebook::{CodeEdit, CodeRow, CoderRow, CodebookVersion, CodingRow, MergeOutcome};
pub use crypto::{file_looks_encrypted, EncryptionOutcome, StaleBackup, CIPHER_DESCRIPTION, MIN_PASSPHRASE_CHARS};
pub use deid::{map_back, DeidAdded, DeidApplied, DeidCopyOutcome, DeidOutcome, DeidTakenOut, DeidPlace, DeidPreview, DeidRule, DeidRuleCount, DeidRulePatch, DeidSample, DeidSkipped, DeidSourceCount, DeidState, Edit as DeidEdit, NewDeidRules, Scrubber, MAX_RULES as MAX_DEID_RULES};
pub use deid_suggest::DeidSuggestion;
pub use edit::EditOutcome;
pub use event::{Event, HistoryEntry};
pub use memo::{LinkRow, MemoRow, NewMemo};
pub use import::{
    AttributeSpec, CaseMember, FormatRun, ImportBatch, ImportOutcome, ImportWarning, ImportedCase, ImportedSource,
    ImportedUnit,
};
pub use model::{AppliesTo, CoderRole, CodingMode, Level, MemoKind, MemoScope, Origin, ProposalOrigin, RunKind, SourceType, UnitKind};
pub use ops::{sha256_hex, NewCode, NewCoder, NewCoding, NewSource};
pub use project::{check_backup_dir, NewProject, OpenOptions, Project, ProjectInfo, DEFAULT_KEEP_BACKUPS, PROJECT_EXTENSION};
pub use template::{built_in_templates, find_template, CodebookIssue, ItemState, Phase, PhaseState, Rules, SeedCode, Template, TemplateState};
pub use undo::UndoOutcome;
pub use segments::{CodedSegment, SegmentFilter};
pub use explore::{
    AlikePair, Bubble, BubbleAxis, BubbleCell, BubbleSpec, Deviation, DeviationGroup, DeviationRow, DeviationSpec, PairPart, SideBySide, SideBySideSpec, SideBySideTopic, SideGroup, Similarity, SimilarityFeature,
    SimilaritySpec, TopicQuote,
    CaseDisplay, CaseDisplaySpec, DisplayCell, DisplayCode, DisplayQuote, DisplayRow, Typology, TypologyCase, TypologyCategory, TypologyCell, TypologyColumn, TypologyGroup, TypologySpec,
    CaseVariable, CodeProfile, CodeSummary, Combine, CountedName, Counts, Matrix, MatrixColumn, MatrixRow, MatrixSpec, Node as QueryNode, QueryCoding, QueryItem, QueryResult, QuerySpec,
    RelationCode, RelationMode, Relations, RelationsSpec, ResultUnit, RowSpec, Scope as ExploreScope,
};
pub use concept_map::{AddedCodes, ConceptMap, MapLink, MapNode, MapRow};
pub use crosstab::{CaseRef, Crosstab, CrosstabAxis, CrosstabSpec, CrosstabValue};
pub use calendar::{day_text, month_text, parse_day, parse_number};
pub use case_table::{CaseTable, CaseTableAttribute, CaseTableCode};
pub use saved::{Frozen, FrozenComparison, SavedQuery, SavedQueryRow};
pub use stat_notebook::{NotebookPage, Rerun};
pub use integration::{
    DesignApply, DesignInput, DesignOutcome, DesignPlan, IntegrationEntry, IntegrationInput, MixedDesign, PlannedCode, PlannedVariable, TopicNumbers, TopicWords, TriangulationEvidence, TriangulationInput,
    TriangulationRow, DESIGN_KINDS,
};
pub use mixed::{QualitizeOutcome, QualitizePlan, QualitizeSpec, QualitizeTakeBack, QualitizeUndo, QualitizeValue};
pub use search::{Query, Scan, SearchFilter, SearchHit, SearchResults};
pub use proposal::{BatchSummary, DecisionOutcome, NewProposal, Passage, ProposalBatch, ProposalRow};
pub use media::{default_recording_description, media_kind, mime_for, recording_fingerprint, MediaChunk, MediaRow, SourceRecording, MAX_CHUNK};
pub use image::{default_description, exif_orientation, image_kind_of_name, picture_details, read_image, sniff, ImageAttachment, ImageData, ImageRow, PictureDetails, IMAGE_MIMES, MAX_IMAGE_BYTES, MAX_IMAGE_SIDE};
pub use dataprep::{date_part, DatePart, GroupFunction, SampleSize};
pub use datatable::{long_to_wide, wide_to_long, SplitGroup, SplitPlan, MAX_SPLIT_GROUPS, DataTable, GroupTable, LongToWide, MergeColumn, MergeSpec, MergeVariablesOutcome, SortKey, Summary, WideToLong, MAX_TABLE_ROWS};
pub use pdf::{PageSizeRow, PdfAttachment, PdfData, PdfRow, SourcePdf, MAX_PDF_BYTES, MAX_PDF_PAGES};
pub use regions::{clock, clock_pair, overlaps, NewRegion, RegionRow, Shape, MAX_POINTS, MAX_SPAN_END_MS, MIN_POINTS, MIN_SIDE, MIN_SPAN_MS};
pub use ai::{AiRunDetail, AiRunRow, AnswerOutcome, Packet, PacketCode, PacketLeftOut, PacketSpec, Rejected, MAX_PASSAGES};
pub use autocode::{speaker_key, FoundSpan, HeadingLevel, KnownDropped, Section, SpeakerGroup, SpeakerTurn};
pub use final_layer::FinalSummary;
pub use region_final::{RegionCase, RegionCounts, RegionDecision, RegionMember};
pub use merge::{CodeConflict, MergeChoices, MergeReport, Skipped, ValueConflict, ValueKey};
pub use packet::{CoderPacketInfo, CoderPacketReport, CoderPacketSpec};
pub use teaching::{kappa_2x2, tidy_students, ClassCollectOutcome, ClassCollectReport, ClassPacketOutcome, ClassPacketsReport, ClassPacketsSpec, ClassReview, ClassReviewSpec, ClassFeedback, CodeReview, FeedbackPassage, HardPassage, StudentFeedback, StudentReview, MAX_STUDENTS};
pub use passage_links::{PassageLink, Relation, COMMON_RELATIONS, MAX_RELATION_CHARS};
pub use privacy::{CoderFact, NotShared, PrivacyFacts};
pub use theme_ai::{ThemeAnswerOutcome, ThemePacket, ThemeRun, ThemeSuggestion, MIN_CODES as MIN_THEME_CODES};
pub use themes::{ThemeCard, ThemeCode, ThemeOverview, ThemeQuote, MAX_QUOTES};
pub use variables::{CaseData, CaseName, CellChange, CodeMeasure, Otherwise, RecodeRule, TransformKind, TransformOutcome, TransformPreview, TransformSpec, VariableDef, VariableSpec};
pub use stat_charts::{BarCategory, BoxGroup, Chart, CurveLine, MeanGroup, Point, Subject};
pub use statistics::{Assumption, BootstrapOptions, Category, Measure, PostHocName, Procedure, ResultGroup, RunsCut, ResultVariable, StatOptions, StatRequest, StatResult, StatTable, StatVariable, StatVariables, Status, TableCase, TableColumn, VariableKind, WeightInfo};
pub use text_analytics::{Context, ContextSpec, Contexts, WordCount, WordFrequencies, WordSpec};
pub use adjudication::{CodeRef, CoderCodes, Decision, DecisionRow, Disagreement};
pub use reliability::{AgreementReport, AgreementReportRecord, AgreementRequest, CodeReport, CoderInfo, RunSummary, Sample, Stratify};
pub use query::{AttributeValueRow, CaseRow, CaseUnitMember, SourceDetail, SourceImage, SourceRow, UnitRef, UnitRow};

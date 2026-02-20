//! Event types and categories for JAM telemetry visualization
//!
//! This module contains:
//! - Event enum matching JIP-3 specification (for storage/parsing)
//! - Event categories for UI grouping
//! - Event names and colors for display

use serde::{Deserialize, Serialize};

// ============================================================================
// Basic Types (subset of jamtart types.rs, no encoding)
// ============================================================================

pub type Timestamp = u64;
pub type EventId = u64;
pub type PeerId = [u8; 32];
pub type Hash = [u8; 32];
pub type HeaderHash = Hash;
pub type WorkPackageHash = Hash;
pub type WorkReportHash = Hash;
pub type ErasureRoot = Hash;
pub type SegmentsRoot = Hash;
pub type Slot = u32;
pub type EpochIndex = u32;
pub type ValidatorIndex = u16;
pub type CoreIndex = u16;
pub type ServiceId = u32;
pub type ShardIndex = u16;
pub type Gas = u64;
pub type TicketId = Hash;
pub type TicketAttempt = u8;
#[allow(dead_code)]
pub type ImportSegmentId = u16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reason(#[allow(dead_code)] pub String);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PeerAddress {
    pub ipv6: [u8; 16],
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDetails {
    pub peer_id: PeerId,
    pub peer_address: PeerAddress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConnectionSide {
    Local = 0,
    Remote = 1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BlockRequestDirection {
    Ascending = 0,
    Descending = 1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GuaranteeDiscardReason {
    PackageReportedOnChain = 0,
    ReplacedByBetter = 1,
    CannotReportOnChain = 2,
    TooManyGuarantees = 3,
    Other = 4,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnnouncedPreimageForgetReason {
    ProvidedOnChain = 0,
    NotRequestedOnChain = 1,
    FailedToAcquire = 2,
    TooManyAnnounced = 3,
    BadLength = 4,
    Other = 5,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PreimageDiscardReason {
    ProvidedOnChain = 0,
    NotRequestedOnChain = 1,
    TooManyPreimages = 2,
    Other = 3,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReconstructionKind {
    NonTrivial = 0,
    Trivial = 1,
}

// Simplified outline types (just the essential fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockOutline {
    pub size_bytes: u32,
    pub hash: HeaderHash,
    pub num_tickets: u32,
    pub num_preimages: u32,
    pub total_preimages_size: u32,
    pub num_guarantees: u32,
    pub num_assurances: u32,
    pub num_dispute_verdicts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkPackageOutline {
    pub work_package_size: u32,
    pub work_package_hash: WorkPackageHash,
    pub anchor: HeaderHash,
    pub lookup_anchor_slot: Slot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkReportOutline {
    pub work_report_hash: WorkReportHash,
    pub bundle_size: u32,
    pub erasure_root: ErasureRoot,
    pub segments_root: SegmentsRoot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuaranteeOutline {
    pub work_report_hash: WorkReportHash,
    pub slot: Slot,
    pub guarantors: Vec<ValidatorIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityStatement {
    pub anchor: HeaderHash,
    pub bitfield: Vec<u8>,
}

// Simplified cost types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecCost {
    pub gas_used: Gas,
    pub elapsed_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsAuthorizedCost {
    pub total: ExecCost,
    pub load_ns: u64,
    pub host_call: ExecCost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineCost {
    pub total: ExecCost,
    pub load_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccumulateCost {
    pub num_calls: u32,
    pub num_transfers: u32,
    pub num_items: u32,
    pub total: ExecCost,
    pub load_ns: u64,
}

// ============================================================================
// Event Type Discriminant
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum EventType {
    // Meta events
    Dropped = 0,

    // Status and sync
    Status = 10,
    BestBlockChanged = 11,
    FinalizedBlockChanged = 12,
    SyncStatusChanged = 13,

    // Connection events
    ConnectionRefused = 20,
    ConnectingIn = 21,
    ConnectInFailed = 22,
    ConnectedIn = 23,
    ConnectingOut = 24,
    ConnectOutFailed = 25,
    ConnectedOut = 26,
    Disconnected = 27,
    PeerMisbehaved = 28,

    // Authoring and importing
    Authoring = 40,
    AuthoringFailed = 41,
    Authored = 42,
    Importing = 43,
    BlockVerificationFailed = 44,
    BlockVerified = 45,
    BlockExecutionFailed = 46,
    BlockExecuted = 47,

    // Block distribution
    BlockAnnouncementStreamOpened = 60,
    BlockAnnouncementStreamClosed = 61,
    BlockAnnounced = 62,
    SendingBlockRequest = 63,
    ReceivingBlockRequest = 64,
    BlockRequestFailed = 65,
    BlockRequestSent = 66,
    BlockRequestReceived = 67,
    BlockTransferred = 68,

    // Ticket generation and transfer
    GeneratingTickets = 80,
    TicketGenerationFailed = 81,
    TicketsGenerated = 82,
    TicketTransferFailed = 83,
    TicketTransferred = 84,

    // Work package submission
    WorkPackageSubmission = 90,
    WorkPackageBeingShared = 91,
    WorkPackageFailed = 92,
    DuplicateWorkPackage = 93,
    WorkPackageReceived = 94,
    Authorized = 95,
    ExtrinsicDataReceived = 96,
    ImportsReceived = 97,
    SharingWorkPackage = 98,
    WorkPackageSharingFailed = 99,
    BundleSent = 100,
    Refined = 101,
    WorkReportBuilt = 102,
    WorkReportSignatureSent = 103,
    WorkReportSignatureReceived = 104,
    GuaranteeBuilt = 105,
    SendingGuarantee = 106,
    GuaranteeSendFailed = 107,
    GuaranteeSent = 108,
    GuaranteesDistributed = 109,
    ReceivingGuarantee = 110,
    GuaranteeReceiveFailed = 111,
    GuaranteeReceived = 112,
    GuaranteeDiscarded = 113,

    // Shard requests
    SendingShardRequest = 120,
    ReceivingShardRequest = 121,
    ShardRequestFailed = 122,
    ShardRequestSent = 123,
    ShardRequestReceived = 124,
    ShardsTransferred = 125,

    // Assurance distribution
    DistributingAssurance = 126,
    AssuranceSendFailed = 127,
    AssuranceSent = 128,
    AssuranceDistributed = 129,
    AssuranceReceiveFailed = 130,
    AssuranceReceived = 131,

    // Bundle shard requests
    SendingBundleShardRequest = 140,
    ReceivingBundleShardRequest = 141,
    BundleShardRequestFailed = 142,
    BundleShardRequestSent = 143,
    BundleShardRequestReceived = 144,
    BundleShardTransferred = 145,
    ReconstructingBundle = 146,
    BundleReconstructed = 147,
    SendingBundleRequest = 148,
    ReceivingBundleRequest = 149,
    BundleRequestFailed = 150,
    BundleRequestSent = 151,
    BundleRequestReceived = 152,
    BundleTransferred = 153,

    // Work package hash mapping
    WorkPackageHashMapped = 160,
    SegmentsRootMapped = 161,

    // Segment shard requests
    SendingSegmentShardRequest = 162,
    ReceivingSegmentShardRequest = 163,
    SegmentShardRequestFailed = 164,
    SegmentShardRequestSent = 165,
    SegmentShardRequestReceived = 166,
    SegmentShardsTransferred = 167,

    // Segment reconstruction
    ReconstructingSegments = 168,
    SegmentReconstructionFailed = 169,
    SegmentsReconstructed = 170,
    SegmentVerificationFailed = 171,
    SegmentsVerified = 172,
    SendingSegmentRequest = 173,
    ReceivingSegmentRequest = 174,
    SegmentRequestFailed = 175,
    SegmentRequestSent = 176,
    SegmentRequestReceived = 177,
    SegmentsTransferred = 178,

    // Preimage events
    PreimageAnnouncementFailed = 190,
    PreimageAnnounced = 191,
    AnnouncedPreimageForgotten = 192,
    SendingPreimageRequest = 193,
    ReceivingPreimageRequest = 194,
    PreimageRequestFailed = 195,
    PreimageRequestSent = 196,
    PreimageRequestReceived = 197,
    PreimageTransferred = 198,
    PreimageDiscarded = 199,
}

impl EventType {
    #[allow(dead_code)]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(EventType::Dropped),
            10 => Some(EventType::Status),
            11 => Some(EventType::BestBlockChanged),
            12 => Some(EventType::FinalizedBlockChanged),
            13 => Some(EventType::SyncStatusChanged),
            // SAFETY: EventType is #[repr(u8)] with contiguous discriminants within each
            // range. The match arms guarantee `value` is a valid discriminant before
            // transmuting, so the resulting enum value is always well-defined.
            20..=28 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            40..=47 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            60..=68 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            80..=84 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            90..=113 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            120..=131 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            140..=153 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            160..=178 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            190..=199 => Some(unsafe { std::mem::transmute::<u8, EventType>(value) }),
            _ => None,
        }
    }
}

// ============================================================================
// Event Enum (full JIP-3 specification)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Meta events
    Dropped {
        timestamp: Timestamp,
        last_timestamp: Timestamp,
        num: u64,
    },

    // Status
    Status {
        timestamp: Timestamp,
        num_peers: u32,
        num_val_peers: u32,
        num_sync_peers: u32,
        num_guarantees: Vec<u8>,
        num_shards: u32,
        shards_size: u64,
        num_preimages: u32,
        preimages_size: u32,
    },

    BestBlockChanged {
        timestamp: Timestamp,
        slot: Slot,
        hash: HeaderHash,
    },

    FinalizedBlockChanged {
        timestamp: Timestamp,
        slot: Slot,
        hash: HeaderHash,
    },

    SyncStatusChanged {
        timestamp: Timestamp,
        synced: bool,
    },

    // Connection events
    ConnectionRefused {
        timestamp: Timestamp,
        from: PeerAddress,
    },

    ConnectingIn {
        timestamp: Timestamp,
        from: PeerAddress,
    },

    ConnectInFailed {
        timestamp: Timestamp,
        connecting_id: EventId,
        reason: Reason,
    },

    ConnectedIn {
        timestamp: Timestamp,
        connecting_id: EventId,
        peer_id: PeerId,
    },

    ConnectingOut {
        timestamp: Timestamp,
        to: PeerDetails,
    },

    ConnectOutFailed {
        timestamp: Timestamp,
        connecting_id: EventId,
        reason: Reason,
    },

    ConnectedOut {
        timestamp: Timestamp,
        connecting_id: EventId,
    },

    Disconnected {
        timestamp: Timestamp,
        peer: PeerId,
        terminator: Option<ConnectionSide>,
        reason: Reason,
    },

    PeerMisbehaved {
        timestamp: Timestamp,
        peer: PeerId,
        reason: Reason,
    },

    // Authoring events
    Authoring {
        timestamp: Timestamp,
        slot: Slot,
        parent: HeaderHash,
    },

    AuthoringFailed {
        timestamp: Timestamp,
        authoring_id: EventId,
        reason: Reason,
    },

    Authored {
        timestamp: Timestamp,
        authoring_id: EventId,
        outline: BlockOutline,
    },

    Importing {
        timestamp: Timestamp,
        slot: Slot,
        outline: BlockOutline,
    },

    BlockVerificationFailed {
        timestamp: Timestamp,
        importing_id: EventId,
        reason: Reason,
    },

    BlockVerified {
        timestamp: Timestamp,
        importing_id: EventId,
    },

    BlockExecutionFailed {
        timestamp: Timestamp,
        authoring_or_importing_id: EventId,
        reason: Reason,
    },

    BlockExecuted {
        timestamp: Timestamp,
        authoring_or_importing_id: EventId,
        accumulate_costs: Vec<(ServiceId, AccumulateCost)>,
    },

    // Block distribution
    BlockAnnouncementStreamOpened {
        timestamp: Timestamp,
        peer: PeerId,
        opener: ConnectionSide,
    },

    BlockAnnouncementStreamClosed {
        timestamp: Timestamp,
        peer: PeerId,
        closer: ConnectionSide,
        reason: Reason,
    },

    BlockAnnounced {
        timestamp: Timestamp,
        peer: PeerId,
        announcer: ConnectionSide,
        slot: Slot,
        hash: HeaderHash,
    },

    SendingBlockRequest {
        timestamp: Timestamp,
        recipient: PeerId,
        hash: HeaderHash,
        direction: BlockRequestDirection,
        max_blocks: u32,
    },

    ReceivingBlockRequest {
        timestamp: Timestamp,
        sender: PeerId,
    },

    BlockRequestFailed {
        timestamp: Timestamp,
        request_id: EventId,
        reason: Reason,
    },

    BlockRequestSent {
        timestamp: Timestamp,
        request_id: EventId,
    },

    BlockRequestReceived {
        timestamp: Timestamp,
        request_id: EventId,
        hash: HeaderHash,
        direction: BlockRequestDirection,
        max_blocks: u32,
    },

    BlockTransferred {
        timestamp: Timestamp,
        request_id: EventId,
        slot: Slot,
        outline: BlockOutline,
        last: bool,
    },

    // Ticket events
    GeneratingTickets {
        timestamp: Timestamp,
        epoch: EpochIndex,
    },

    TicketGenerationFailed {
        timestamp: Timestamp,
        generating_id: EventId,
        reason: Reason,
    },

    TicketsGenerated {
        timestamp: Timestamp,
        generating_id: EventId,
        ids: Vec<TicketId>,
    },

    TicketTransferFailed {
        timestamp: Timestamp,
        peer: PeerId,
        sender: ConnectionSide,
        from_proxy: bool,
        reason: Reason,
    },

    TicketTransferred {
        timestamp: Timestamp,
        peer: PeerId,
        sender: ConnectionSide,
        from_proxy: bool,
        epoch: EpochIndex,
        attempt: TicketAttempt,
        id: TicketId,
    },

    // Work package events (simplified - add full fields as needed)
    WorkPackageSubmission {
        timestamp: Timestamp,
        builder: PeerId,
        bundle: bool,
    },

    WorkPackageBeingShared {
        timestamp: Timestamp,
        primary: PeerId,
    },

    WorkPackageFailed {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        reason: Reason,
    },

    DuplicateWorkPackage {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        core: CoreIndex,
        hash: WorkPackageHash,
    },

    WorkPackageReceived {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        core: CoreIndex,
        outline: WorkPackageOutline,
    },

    Authorized {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        cost: IsAuthorizedCost,
    },

    ExtrinsicDataReceived {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
    },

    ImportsReceived {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
    },

    SharingWorkPackage {
        timestamp: Timestamp,
        submission_id: EventId,
        secondary: PeerId,
    },

    WorkPackageSharingFailed {
        timestamp: Timestamp,
        submission_id: EventId,
        secondary: PeerId,
        reason: Reason,
    },

    BundleSent {
        timestamp: Timestamp,
        submission_id: EventId,
        secondary: PeerId,
    },

    Refined {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        costs: Vec<RefineCost>,
    },

    WorkReportBuilt {
        timestamp: Timestamp,
        submission_or_share_id: EventId,
        outline: WorkReportOutline,
    },

    WorkReportSignatureSent {
        timestamp: Timestamp,
        share_id: EventId,
    },

    WorkReportSignatureReceived {
        timestamp: Timestamp,
        submission_id: EventId,
        secondary: PeerId,
    },

    GuaranteeBuilt {
        timestamp: Timestamp,
        submission_id: EventId,
        outline: GuaranteeOutline,
    },

    SendingGuarantee {
        timestamp: Timestamp,
        built_id: EventId,
        recipient: PeerId,
    },

    GuaranteeSendFailed {
        timestamp: Timestamp,
        sending_id: EventId,
        reason: Reason,
    },

    GuaranteeSent {
        timestamp: Timestamp,
        sending_id: EventId,
    },

    GuaranteesDistributed {
        timestamp: Timestamp,
        submission_id: EventId,
    },

    ReceivingGuarantee {
        timestamp: Timestamp,
        sender: PeerId,
    },

    GuaranteeReceiveFailed {
        timestamp: Timestamp,
        receiving_id: EventId,
        reason: Reason,
    },

    GuaranteeReceived {
        timestamp: Timestamp,
        receiving_id: EventId,
        outline: GuaranteeOutline,
    },

    GuaranteeDiscarded {
        timestamp: Timestamp,
        outline: GuaranteeOutline,
        reason: GuaranteeDiscardReason,
    },

    // Availability distribution (simplified)
    SendingShardRequest {
        timestamp: Timestamp,
        guarantor: PeerId,
        erasure_root: ErasureRoot,
        shard: ShardIndex,
    },

    ReceivingShardRequest {
        timestamp: Timestamp,
        assurer: PeerId,
    },

    ShardRequestFailed {
        timestamp: Timestamp,
        request_id: EventId,
        reason: Reason,
    },

    ShardRequestSent {
        timestamp: Timestamp,
        request_id: EventId,
    },

    ShardRequestReceived {
        timestamp: Timestamp,
        request_id: EventId,
        erasure_root: ErasureRoot,
        shard: ShardIndex,
    },

    ShardsTransferred {
        timestamp: Timestamp,
        request_id: EventId,
    },

    DistributingAssurance {
        timestamp: Timestamp,
        statement: AvailabilityStatement,
    },

    AssuranceSendFailed {
        timestamp: Timestamp,
        distributing_id: EventId,
        recipient: PeerId,
        reason: Reason,
    },

    AssuranceSent {
        timestamp: Timestamp,
        distributing_id: EventId,
        recipient: PeerId,
    },

    AssuranceDistributed {
        timestamp: Timestamp,
        distributing_id: EventId,
    },

    AssuranceReceiveFailed {
        timestamp: Timestamp,
        sender: PeerId,
        reason: Reason,
    },

    AssuranceReceived {
        timestamp: Timestamp,
        sender: PeerId,
        anchor: HeaderHash,
    },

    // Bundle recovery (simplified stubs)
    SendingBundleShardRequest { timestamp: Timestamp },
    ReceivingBundleShardRequest { timestamp: Timestamp },
    BundleShardRequestFailed { timestamp: Timestamp },
    BundleShardRequestSent { timestamp: Timestamp },
    BundleShardRequestReceived { timestamp: Timestamp },
    BundleShardTransferred { timestamp: Timestamp },
    ReconstructingBundle { timestamp: Timestamp },
    BundleReconstructed { timestamp: Timestamp },
    SendingBundleRequest { timestamp: Timestamp },
    ReceivingBundleRequest { timestamp: Timestamp },
    BundleRequestFailed { timestamp: Timestamp },
    BundleRequestSent { timestamp: Timestamp },
    BundleRequestReceived { timestamp: Timestamp },
    BundleTransferred { timestamp: Timestamp },

    // Segment events (simplified stubs)
    WorkPackageHashMapped { timestamp: Timestamp },
    SegmentsRootMapped { timestamp: Timestamp },
    SendingSegmentShardRequest { timestamp: Timestamp },
    ReceivingSegmentShardRequest { timestamp: Timestamp },
    SegmentShardRequestFailed { timestamp: Timestamp },
    SegmentShardRequestSent { timestamp: Timestamp },
    SegmentShardRequestReceived { timestamp: Timestamp },
    SegmentShardsTransferred { timestamp: Timestamp },
    ReconstructingSegments { timestamp: Timestamp },
    SegmentReconstructionFailed { timestamp: Timestamp },
    SegmentsReconstructed { timestamp: Timestamp },
    SegmentVerificationFailed { timestamp: Timestamp },
    SegmentsVerified { timestamp: Timestamp },
    SendingSegmentRequest { timestamp: Timestamp },
    ReceivingSegmentRequest { timestamp: Timestamp },
    SegmentRequestFailed { timestamp: Timestamp },
    SegmentRequestSent { timestamp: Timestamp },
    SegmentRequestReceived { timestamp: Timestamp },
    SegmentsTransferred { timestamp: Timestamp },

    // Preimage events (simplified stubs)
    PreimageAnnouncementFailed { timestamp: Timestamp },
    PreimageAnnounced { timestamp: Timestamp },
    AnnouncedPreimageForgotten { timestamp: Timestamp },
    SendingPreimageRequest { timestamp: Timestamp },
    ReceivingPreimageRequest { timestamp: Timestamp },
    PreimageRequestFailed { timestamp: Timestamp },
    PreimageRequestSent { timestamp: Timestamp },
    PreimageRequestReceived { timestamp: Timestamp },
    PreimageTransferred { timestamp: Timestamp },
    PreimageDiscarded { timestamp: Timestamp },
}

impl Event {
    /// Get the event type discriminant
    pub fn event_type(&self) -> EventType {
        match self {
            Event::Dropped { .. } => EventType::Dropped,
            Event::Status { .. } => EventType::Status,
            Event::BestBlockChanged { .. } => EventType::BestBlockChanged,
            Event::FinalizedBlockChanged { .. } => EventType::FinalizedBlockChanged,
            Event::SyncStatusChanged { .. } => EventType::SyncStatusChanged,
            Event::ConnectionRefused { .. } => EventType::ConnectionRefused,
            Event::ConnectingIn { .. } => EventType::ConnectingIn,
            Event::ConnectInFailed { .. } => EventType::ConnectInFailed,
            Event::ConnectedIn { .. } => EventType::ConnectedIn,
            Event::ConnectingOut { .. } => EventType::ConnectingOut,
            Event::ConnectOutFailed { .. } => EventType::ConnectOutFailed,
            Event::ConnectedOut { .. } => EventType::ConnectedOut,
            Event::Disconnected { .. } => EventType::Disconnected,
            Event::PeerMisbehaved { .. } => EventType::PeerMisbehaved,
            Event::Authoring { .. } => EventType::Authoring,
            Event::AuthoringFailed { .. } => EventType::AuthoringFailed,
            Event::Authored { .. } => EventType::Authored,
            Event::Importing { .. } => EventType::Importing,
            Event::BlockVerificationFailed { .. } => EventType::BlockVerificationFailed,
            Event::BlockVerified { .. } => EventType::BlockVerified,
            Event::BlockExecutionFailed { .. } => EventType::BlockExecutionFailed,
            Event::BlockExecuted { .. } => EventType::BlockExecuted,
            Event::BlockAnnouncementStreamOpened { .. } => EventType::BlockAnnouncementStreamOpened,
            Event::BlockAnnouncementStreamClosed { .. } => EventType::BlockAnnouncementStreamClosed,
            Event::BlockAnnounced { .. } => EventType::BlockAnnounced,
            Event::SendingBlockRequest { .. } => EventType::SendingBlockRequest,
            Event::ReceivingBlockRequest { .. } => EventType::ReceivingBlockRequest,
            Event::BlockRequestFailed { .. } => EventType::BlockRequestFailed,
            Event::BlockRequestSent { .. } => EventType::BlockRequestSent,
            Event::BlockRequestReceived { .. } => EventType::BlockRequestReceived,
            Event::BlockTransferred { .. } => EventType::BlockTransferred,
            Event::GeneratingTickets { .. } => EventType::GeneratingTickets,
            Event::TicketGenerationFailed { .. } => EventType::TicketGenerationFailed,
            Event::TicketsGenerated { .. } => EventType::TicketsGenerated,
            Event::TicketTransferFailed { .. } => EventType::TicketTransferFailed,
            Event::TicketTransferred { .. } => EventType::TicketTransferred,
            Event::WorkPackageSubmission { .. } => EventType::WorkPackageSubmission,
            Event::WorkPackageBeingShared { .. } => EventType::WorkPackageBeingShared,
            Event::WorkPackageFailed { .. } => EventType::WorkPackageFailed,
            Event::DuplicateWorkPackage { .. } => EventType::DuplicateWorkPackage,
            Event::WorkPackageReceived { .. } => EventType::WorkPackageReceived,
            Event::Authorized { .. } => EventType::Authorized,
            Event::ExtrinsicDataReceived { .. } => EventType::ExtrinsicDataReceived,
            Event::ImportsReceived { .. } => EventType::ImportsReceived,
            Event::SharingWorkPackage { .. } => EventType::SharingWorkPackage,
            Event::WorkPackageSharingFailed { .. } => EventType::WorkPackageSharingFailed,
            Event::BundleSent { .. } => EventType::BundleSent,
            Event::Refined { .. } => EventType::Refined,
            Event::WorkReportBuilt { .. } => EventType::WorkReportBuilt,
            Event::WorkReportSignatureSent { .. } => EventType::WorkReportSignatureSent,
            Event::WorkReportSignatureReceived { .. } => EventType::WorkReportSignatureReceived,
            Event::GuaranteeBuilt { .. } => EventType::GuaranteeBuilt,
            Event::SendingGuarantee { .. } => EventType::SendingGuarantee,
            Event::GuaranteeSendFailed { .. } => EventType::GuaranteeSendFailed,
            Event::GuaranteeSent { .. } => EventType::GuaranteeSent,
            Event::GuaranteesDistributed { .. } => EventType::GuaranteesDistributed,
            Event::ReceivingGuarantee { .. } => EventType::ReceivingGuarantee,
            Event::GuaranteeReceiveFailed { .. } => EventType::GuaranteeReceiveFailed,
            Event::GuaranteeReceived { .. } => EventType::GuaranteeReceived,
            Event::GuaranteeDiscarded { .. } => EventType::GuaranteeDiscarded,
            Event::SendingShardRequest { .. } => EventType::SendingShardRequest,
            Event::ReceivingShardRequest { .. } => EventType::ReceivingShardRequest,
            Event::ShardRequestFailed { .. } => EventType::ShardRequestFailed,
            Event::ShardRequestSent { .. } => EventType::ShardRequestSent,
            Event::ShardRequestReceived { .. } => EventType::ShardRequestReceived,
            Event::ShardsTransferred { .. } => EventType::ShardsTransferred,
            Event::DistributingAssurance { .. } => EventType::DistributingAssurance,
            Event::AssuranceSendFailed { .. } => EventType::AssuranceSendFailed,
            Event::AssuranceSent { .. } => EventType::AssuranceSent,
            Event::AssuranceDistributed { .. } => EventType::AssuranceDistributed,
            Event::AssuranceReceiveFailed { .. } => EventType::AssuranceReceiveFailed,
            Event::AssuranceReceived { .. } => EventType::AssuranceReceived,
            Event::SendingBundleShardRequest { .. } => EventType::SendingBundleShardRequest,
            Event::ReceivingBundleShardRequest { .. } => EventType::ReceivingBundleShardRequest,
            Event::BundleShardRequestFailed { .. } => EventType::BundleShardRequestFailed,
            Event::BundleShardRequestSent { .. } => EventType::BundleShardRequestSent,
            Event::BundleShardRequestReceived { .. } => EventType::BundleShardRequestReceived,
            Event::BundleShardTransferred { .. } => EventType::BundleShardTransferred,
            Event::ReconstructingBundle { .. } => EventType::ReconstructingBundle,
            Event::BundleReconstructed { .. } => EventType::BundleReconstructed,
            Event::SendingBundleRequest { .. } => EventType::SendingBundleRequest,
            Event::ReceivingBundleRequest { .. } => EventType::ReceivingBundleRequest,
            Event::BundleRequestFailed { .. } => EventType::BundleRequestFailed,
            Event::BundleRequestSent { .. } => EventType::BundleRequestSent,
            Event::BundleRequestReceived { .. } => EventType::BundleRequestReceived,
            Event::BundleTransferred { .. } => EventType::BundleTransferred,
            Event::WorkPackageHashMapped { .. } => EventType::WorkPackageHashMapped,
            Event::SegmentsRootMapped { .. } => EventType::SegmentsRootMapped,
            Event::SendingSegmentShardRequest { .. } => EventType::SendingSegmentShardRequest,
            Event::ReceivingSegmentShardRequest { .. } => EventType::ReceivingSegmentShardRequest,
            Event::SegmentShardRequestFailed { .. } => EventType::SegmentShardRequestFailed,
            Event::SegmentShardRequestSent { .. } => EventType::SegmentShardRequestSent,
            Event::SegmentShardRequestReceived { .. } => EventType::SegmentShardRequestReceived,
            Event::SegmentShardsTransferred { .. } => EventType::SegmentShardsTransferred,
            Event::ReconstructingSegments { .. } => EventType::ReconstructingSegments,
            Event::SegmentReconstructionFailed { .. } => EventType::SegmentReconstructionFailed,
            Event::SegmentsReconstructed { .. } => EventType::SegmentsReconstructed,
            Event::SegmentVerificationFailed { .. } => EventType::SegmentVerificationFailed,
            Event::SegmentsVerified { .. } => EventType::SegmentsVerified,
            Event::SendingSegmentRequest { .. } => EventType::SendingSegmentRequest,
            Event::ReceivingSegmentRequest { .. } => EventType::ReceivingSegmentRequest,
            Event::SegmentRequestFailed { .. } => EventType::SegmentRequestFailed,
            Event::SegmentRequestSent { .. } => EventType::SegmentRequestSent,
            Event::SegmentRequestReceived { .. } => EventType::SegmentRequestReceived,
            Event::SegmentsTransferred { .. } => EventType::SegmentsTransferred,
            Event::PreimageAnnouncementFailed { .. } => EventType::PreimageAnnouncementFailed,
            Event::PreimageAnnounced { .. } => EventType::PreimageAnnounced,
            Event::AnnouncedPreimageForgotten { .. } => EventType::AnnouncedPreimageForgotten,
            Event::SendingPreimageRequest { .. } => EventType::SendingPreimageRequest,
            Event::ReceivingPreimageRequest { .. } => EventType::ReceivingPreimageRequest,
            Event::PreimageRequestFailed { .. } => EventType::PreimageRequestFailed,
            Event::PreimageRequestSent { .. } => EventType::PreimageRequestSent,
            Event::PreimageRequestReceived { .. } => EventType::PreimageRequestReceived,
            Event::PreimageTransferred { .. } => EventType::PreimageTransferred,
            Event::PreimageDiscarded { .. } => EventType::PreimageDiscarded,
        }
    }

    /// Get the timestamp from any event
    #[allow(dead_code)]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Event::Dropped { timestamp, .. }
            | Event::Status { timestamp, .. }
            | Event::BestBlockChanged { timestamp, .. }
            | Event::FinalizedBlockChanged { timestamp, .. }
            | Event::SyncStatusChanged { timestamp, .. }
            | Event::ConnectionRefused { timestamp, .. }
            | Event::ConnectingIn { timestamp, .. }
            | Event::ConnectInFailed { timestamp, .. }
            | Event::ConnectedIn { timestamp, .. }
            | Event::ConnectingOut { timestamp, .. }
            | Event::ConnectOutFailed { timestamp, .. }
            | Event::ConnectedOut { timestamp, .. }
            | Event::Disconnected { timestamp, .. }
            | Event::PeerMisbehaved { timestamp, .. }
            | Event::Authoring { timestamp, .. }
            | Event::AuthoringFailed { timestamp, .. }
            | Event::Authored { timestamp, .. }
            | Event::Importing { timestamp, .. }
            | Event::BlockVerificationFailed { timestamp, .. }
            | Event::BlockVerified { timestamp, .. }
            | Event::BlockExecutionFailed { timestamp, .. }
            | Event::BlockExecuted { timestamp, .. }
            | Event::BlockAnnouncementStreamOpened { timestamp, .. }
            | Event::BlockAnnouncementStreamClosed { timestamp, .. }
            | Event::BlockAnnounced { timestamp, .. }
            | Event::SendingBlockRequest { timestamp, .. }
            | Event::ReceivingBlockRequest { timestamp, .. }
            | Event::BlockRequestFailed { timestamp, .. }
            | Event::BlockRequestSent { timestamp, .. }
            | Event::BlockRequestReceived { timestamp, .. }
            | Event::BlockTransferred { timestamp, .. }
            | Event::GeneratingTickets { timestamp, .. }
            | Event::TicketGenerationFailed { timestamp, .. }
            | Event::TicketsGenerated { timestamp, .. }
            | Event::TicketTransferFailed { timestamp, .. }
            | Event::TicketTransferred { timestamp, .. }
            | Event::WorkPackageSubmission { timestamp, .. }
            | Event::WorkPackageBeingShared { timestamp, .. }
            | Event::WorkPackageFailed { timestamp, .. }
            | Event::DuplicateWorkPackage { timestamp, .. }
            | Event::WorkPackageReceived { timestamp, .. }
            | Event::Authorized { timestamp, .. }
            | Event::ExtrinsicDataReceived { timestamp, .. }
            | Event::ImportsReceived { timestamp, .. }
            | Event::SharingWorkPackage { timestamp, .. }
            | Event::WorkPackageSharingFailed { timestamp, .. }
            | Event::BundleSent { timestamp, .. }
            | Event::Refined { timestamp, .. }
            | Event::WorkReportBuilt { timestamp, .. }
            | Event::WorkReportSignatureSent { timestamp, .. }
            | Event::WorkReportSignatureReceived { timestamp, .. }
            | Event::GuaranteeBuilt { timestamp, .. }
            | Event::SendingGuarantee { timestamp, .. }
            | Event::GuaranteeSendFailed { timestamp, .. }
            | Event::GuaranteeSent { timestamp, .. }
            | Event::GuaranteesDistributed { timestamp, .. }
            | Event::ReceivingGuarantee { timestamp, .. }
            | Event::GuaranteeReceiveFailed { timestamp, .. }
            | Event::GuaranteeReceived { timestamp, .. }
            | Event::GuaranteeDiscarded { timestamp, .. }
            | Event::SendingShardRequest { timestamp, .. }
            | Event::ReceivingShardRequest { timestamp, .. }
            | Event::ShardRequestFailed { timestamp, .. }
            | Event::ShardRequestSent { timestamp, .. }
            | Event::ShardRequestReceived { timestamp, .. }
            | Event::ShardsTransferred { timestamp, .. }
            | Event::DistributingAssurance { timestamp, .. }
            | Event::AssuranceSendFailed { timestamp, .. }
            | Event::AssuranceSent { timestamp, .. }
            | Event::AssuranceDistributed { timestamp, .. }
            | Event::AssuranceReceiveFailed { timestamp, .. }
            | Event::AssuranceReceived { timestamp, .. }
            | Event::SendingBundleShardRequest { timestamp, .. }
            | Event::ReceivingBundleShardRequest { timestamp, .. }
            | Event::BundleShardRequestFailed { timestamp, .. }
            | Event::BundleShardRequestSent { timestamp, .. }
            | Event::BundleShardRequestReceived { timestamp, .. }
            | Event::BundleShardTransferred { timestamp, .. }
            | Event::ReconstructingBundle { timestamp, .. }
            | Event::BundleReconstructed { timestamp, .. }
            | Event::SendingBundleRequest { timestamp, .. }
            | Event::ReceivingBundleRequest { timestamp, .. }
            | Event::BundleRequestFailed { timestamp, .. }
            | Event::BundleRequestSent { timestamp, .. }
            | Event::BundleRequestReceived { timestamp, .. }
            | Event::BundleTransferred { timestamp, .. }
            | Event::WorkPackageHashMapped { timestamp, .. }
            | Event::SegmentsRootMapped { timestamp, .. }
            | Event::SendingSegmentShardRequest { timestamp, .. }
            | Event::ReceivingSegmentShardRequest { timestamp, .. }
            | Event::SegmentShardRequestFailed { timestamp, .. }
            | Event::SegmentShardRequestSent { timestamp, .. }
            | Event::SegmentShardRequestReceived { timestamp, .. }
            | Event::SegmentShardsTransferred { timestamp, .. }
            | Event::ReconstructingSegments { timestamp, .. }
            | Event::SegmentReconstructionFailed { timestamp, .. }
            | Event::SegmentsReconstructed { timestamp, .. }
            | Event::SegmentVerificationFailed { timestamp, .. }
            | Event::SegmentsVerified { timestamp, .. }
            | Event::SendingSegmentRequest { timestamp, .. }
            | Event::ReceivingSegmentRequest { timestamp, .. }
            | Event::SegmentRequestFailed { timestamp, .. }
            | Event::SegmentRequestSent { timestamp, .. }
            | Event::SegmentRequestReceived { timestamp, .. }
            | Event::SegmentsTransferred { timestamp, .. }
            | Event::PreimageAnnouncementFailed { timestamp, .. }
            | Event::PreimageAnnounced { timestamp, .. }
            | Event::AnnouncedPreimageForgotten { timestamp, .. }
            | Event::SendingPreimageRequest { timestamp, .. }
            | Event::ReceivingPreimageRequest { timestamp, .. }
            | Event::PreimageRequestFailed { timestamp, .. }
            | Event::PreimageRequestSent { timestamp, .. }
            | Event::PreimageRequestReceived { timestamp, .. }
            | Event::PreimageTransferred { timestamp, .. }
            | Event::PreimageDiscarded { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// Event Categories (for UI grouping)
// ============================================================================

#[allow(dead_code)]
pub struct EventCategory {
    pub name: &'static str,
    pub event_types: &'static [u8],
}

#[allow(dead_code)]
pub const EVENT_CATEGORIES: &[EventCategory] = &[
    EventCategory {
        name: "Meta",
        event_types: &[0],
    },
    EventCategory {
        name: "Status",
        event_types: &[10, 11, 12, 13],
    },
    EventCategory {
        name: "Connection",
        event_types: &[20, 21, 22, 23, 24, 25, 26, 27, 28],
    },
    EventCategory {
        name: "Block Auth/Import",
        event_types: &[40, 41, 42, 43, 44, 45, 46, 47],
    },
    EventCategory {
        name: "Block Distribution",
        event_types: &[60, 61, 62, 63, 64, 65, 66, 67, 68],
    },
    EventCategory {
        name: "Safrole Tickets",
        event_types: &[80, 81, 82, 83, 84],
    },
    EventCategory {
        name: "Work Package",
        event_types: &[90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104],
    },
    EventCategory {
        name: "Guaranteeing",
        event_types: &[105, 106, 107, 108, 109, 110, 111, 112, 113],
    },
    EventCategory {
        name: "Availability",
        event_types: &[120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131],
    },
    EventCategory {
        name: "Bundle Recovery",
        event_types: &[140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153],
    },
    EventCategory {
        name: "Segment Recovery",
        event_types: &[
            160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176,
            177, 178,
        ],
    },
    EventCategory {
        name: "Preimages",
        event_types: &[190, 191, 192, 193, 194, 195, 196, 197, 198, 199],
    },
];

/// Event types representing errors, failures, disconnections, and discards.
pub const ERROR_EVENT_TYPES: &[u8] = &[
    // Meta
    0,   // Dropped
    // Connection
    20,  // ConnectionRefused
    22,  // ConnectInFailed
    25,  // ConnectOutFailed
    27,  // Disconnected
    28,  // PeerMisbehaved
    // Block Auth/Import
    41,  // AuthoringFailed
    44,  // BlockVerificationFailed
    46,  // BlockExecutionFailed
    // Block Distribution
    61,  // BlockAnnouncementStreamClosed
    65,  // BlockRequestFailed
    // Safrole Tickets
    81,  // TicketGenerationFailed
    83,  // TicketTransferFailed
    // Work Package
    92,  // WorkPackageFailed
    93,  // DuplicateWorkPackage
    99,  // WorkPackageSharingFailed
    // Guaranteeing
    107, // GuaranteeSendFailed
    111, // GuaranteeReceiveFailed
    113, // GuaranteeDiscarded
    // Availability
    122, // ShardRequestFailed
    127, // AssuranceSendFailed
    130, // AssuranceReceiveFailed
    // Bundle Recovery
    142, // BundleShardRequestFailed
    150, // BundleRequestFailed
    // Segment Recovery
    164, // SegmentShardRequestFailed
    169, // SegmentReconstructionFailed
    171, // SegmentVerificationFailed
    175, // SegmentRequestFailed
    // Preimages
    190, // PreimageAnnouncementFailed
    195, // PreimageRequestFailed
    199, // PreimageDiscarded
];

/// Get the human-readable name for an event type
#[allow(dead_code)]
pub fn event_name(event_type: u8) -> &'static str {
    match event_type {
        0 => "Dropped",
        10 => "Status",
        11 => "BestBlockChanged",
        12 => "FinalizedBlockChanged",
        13 => "SyncStatusChanged",
        20 => "ConnectionRefused",
        21 => "ConnectingIn",
        22 => "ConnectInFailed",
        23 => "ConnectedIn",
        24 => "ConnectingOut",
        25 => "ConnectOutFailed",
        26 => "ConnectedOut",
        27 => "Disconnected",
        28 => "PeerMisbehaved",
        40 => "Authoring",
        41 => "AuthoringFailed",
        42 => "Authored",
        43 => "Importing",
        44 => "BlockVerificationFailed",
        45 => "BlockVerified",
        46 => "BlockExecutionFailed",
        47 => "BlockExecuted",
        60 => "BlockAnnouncementStreamOpened",
        61 => "BlockAnnouncementStreamClosed",
        62 => "BlockAnnounced",
        63 => "SendingBlockRequest",
        64 => "ReceivingBlockRequest",
        65 => "BlockRequestFailed",
        66 => "BlockRequestSent",
        67 => "BlockRequestReceived",
        68 => "BlockTransferred",
        80 => "GeneratingTickets",
        81 => "TicketGenerationFailed",
        82 => "TicketsGenerated",
        83 => "TicketTransferFailed",
        84 => "TicketTransferred",
        90 => "WorkPackageSubmission",
        91 => "WorkPackageBeingShared",
        92 => "WorkPackageFailed",
        93 => "DuplicateWorkPackage",
        94 => "WorkPackageReceived",
        95 => "Authorized",
        96 => "ExtrinsicDataReceived",
        97 => "ImportsReceived",
        98 => "SharingWorkPackage",
        99 => "WorkPackageSharingFailed",
        100 => "BundleSent",
        101 => "Refined",
        102 => "WorkReportBuilt",
        103 => "WorkReportSignatureSent",
        104 => "WorkReportSignatureReceived",
        105 => "GuaranteeBuilt",
        106 => "SendingGuarantee",
        107 => "GuaranteeSendFailed",
        108 => "GuaranteeSent",
        109 => "GuaranteesDistributed",
        110 => "ReceivingGuarantee",
        111 => "GuaranteeReceiveFailed",
        112 => "GuaranteeReceived",
        113 => "GuaranteeDiscarded",
        120 => "SendingShardRequest",
        121 => "ReceivingShardRequest",
        122 => "ShardRequestFailed",
        123 => "ShardRequestSent",
        124 => "ShardRequestReceived",
        125 => "ShardsTransferred",
        126 => "DistributingAssurance",
        127 => "AssuranceSendFailed",
        128 => "AssuranceSent",
        129 => "AssuranceDistributed",
        130 => "AssuranceReceiveFailed",
        131 => "AssuranceReceived",
        140 => "SendingBundleShardRequest",
        141 => "ReceivingBundleShardRequest",
        142 => "BundleShardRequestFailed",
        143 => "BundleShardRequestSent",
        144 => "BundleShardRequestReceived",
        145 => "BundleShardTransferred",
        146 => "ReconstructingBundle",
        147 => "BundleReconstructed",
        148 => "SendingBundleRequest",
        149 => "ReceivingBundleRequest",
        150 => "BundleRequestFailed",
        151 => "BundleRequestSent",
        152 => "BundleRequestReceived",
        153 => "BundleTransferred",
        160 => "WorkPackageHashMapped",
        161 => "SegmentsRootMapped",
        162 => "SendingSegmentShardRequest",
        163 => "ReceivingSegmentShardRequest",
        164 => "SegmentShardRequestFailed",
        165 => "SegmentShardRequestSent",
        166 => "SegmentShardRequestReceived",
        167 => "SegmentShardsTransferred",
        168 => "ReconstructingSegments",
        169 => "SegmentReconstructionFailed",
        170 => "SegmentsReconstructed",
        171 => "SegmentVerificationFailed",
        172 => "SegmentsVerified",
        173 => "SendingSegmentRequest",
        174 => "ReceivingSegmentRequest",
        175 => "SegmentRequestFailed",
        176 => "SegmentRequestSent",
        177 => "SegmentRequestReceived",
        178 => "SegmentsTransferred",
        190 => "PreimageAnnouncementFailed",
        191 => "PreimageAnnounced",
        192 => "AnnouncedPreimageForgotten",
        193 => "SendingPreimageRequest",
        194 => "ReceivingPreimageRequest",
        195 => "PreimageRequestFailed",
        196 => "PreimageRequestSent",
        197 => "PreimageRequestReceived",
        198 => "PreimageTransferred",
        199 => "PreimageDiscarded",
        _ => "Unknown",
    }
}

/// Get color for event type (for visualization) as RGB tuple
#[allow(dead_code)]
pub fn event_color_rgb(event_type: u8) -> (u8, u8, u8) {
    match event_type {
        0 => (128, 128, 128),      // Meta - gray
        10..=13 => (100, 200, 100), // Status - green
        20..=28 => (100, 150, 255), // Connection - blue
        40..=47 => (255, 200, 100), // Block auth - orange
        60..=68 => (200, 100, 255), // Block dist - purple
        80..=84 => (255, 100, 100), // Tickets - red
        90..=104 => (100, 255, 200), // Work Package - cyan
        105..=113 => (255, 100, 200), // Guaranteeing - magenta
        120..=131 => (255, 255, 100), // Availability - yellow
        140..=153 => (255, 150, 150), // Bundle - pink
        160..=178 => (150, 200, 255), // Segment - light blue
        190..=199 => (200, 200, 200), // Preimage - light gray
        _ => (255, 255, 255),
    }
}

// ============================================================================
// DirectedEvent trait - Extract peer direction from events
// ============================================================================

/// Information about a directed event (node-to-node communication)
#[derive(Debug, Clone, Copy)]
pub struct DirectedPeer<'a> {
    /// The peer ID involved in this event
    pub peer_id: &'a PeerId,
    /// True if this node is sending (outbound), false if receiving (inbound)
    pub is_outbound: bool,
}

impl Event {
    /// Extract directed peer information from events that have node-to-node direction.
    /// Returns (peer_id, is_outbound) where is_outbound means this node is the sender.
    /// Returns None for non-directed events.
    pub fn directed_peer(&self) -> Option<DirectedPeer<'_>> {
        match self {
            // === Networking (outbound) ===
            Event::ConnectingOut { to, .. } => Some(DirectedPeer {
                peer_id: &to.peer_id,
                is_outbound: true,
            }),

            // === Networking (inbound) ===
            Event::ConnectedIn { peer_id, .. } => Some(DirectedPeer {
                peer_id,
                is_outbound: false,
            }),

            // === Networking (bidirectional - treat as about the peer) ===
            Event::Disconnected { peer, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: true, // Arbitrary, could be either
            }),
            Event::PeerMisbehaved { peer, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: true, // About the peer
            }),

            // === Block distribution (outbound) ===
            Event::SendingBlockRequest { recipient, .. } => Some(DirectedPeer {
                peer_id: recipient,
                is_outbound: true,
            }),

            // === Block distribution (inbound) ===
            Event::ReceivingBlockRequest { sender, .. } => Some(DirectedPeer {
                peer_id: sender,
                is_outbound: false,
            }),

            // === Block distribution (ConnectionSide-based) ===
            Event::BlockAnnouncementStreamOpened { peer, opener, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: matches!(opener, ConnectionSide::Local),
            }),
            Event::BlockAnnouncementStreamClosed { peer, closer, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: matches!(closer, ConnectionSide::Local),
            }),
            Event::BlockAnnounced { peer, announcer, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: matches!(announcer, ConnectionSide::Local),
            }),

            // === Tickets (ConnectionSide-based) ===
            Event::TicketTransferFailed { peer, sender, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: matches!(sender, ConnectionSide::Local),
            }),
            Event::TicketTransferred { peer, sender, .. } => Some(DirectedPeer {
                peer_id: peer,
                is_outbound: matches!(sender, ConnectionSide::Local),
            }),

            // === Work Package (inbound from builder/primary) ===
            // WorkPackageSubmission is visualized as a collapsing pulse, not a directed particle.
            Event::WorkPackageBeingShared { primary, .. } => Some(DirectedPeer {
                peer_id: primary,
                is_outbound: false,
            }),

            // === Work Package sharing (outbound to secondary) ===
            Event::SharingWorkPackage { secondary, .. } => Some(DirectedPeer {
                peer_id: secondary,
                is_outbound: true,
            }),
            Event::WorkPackageSharingFailed { secondary, .. } => Some(DirectedPeer {
                peer_id: secondary,
                is_outbound: true,
            }),
            Event::BundleSent { secondary, .. } => Some(DirectedPeer {
                peer_id: secondary,
                is_outbound: true,
            }),

            // === Work Report signature (inbound from secondary) ===
            Event::WorkReportSignatureReceived { secondary, .. } => Some(DirectedPeer {
                peer_id: secondary,
                is_outbound: false,
            }),

            // === Guarantee distribution (outbound) ===
            Event::SendingGuarantee { recipient, .. } => Some(DirectedPeer {
                peer_id: recipient,
                is_outbound: true,
            }),

            // === Guarantee distribution (inbound) ===
            Event::ReceivingGuarantee { sender, .. } => Some(DirectedPeer {
                peer_id: sender,
                is_outbound: false,
            }),

            // === Shard requests (outbound to guarantor) ===
            Event::SendingShardRequest { guarantor, .. } => Some(DirectedPeer {
                peer_id: guarantor,
                is_outbound: true,
            }),

            // === Shard requests (inbound from assurer) ===
            Event::ReceivingShardRequest { assurer, .. } => Some(DirectedPeer {
                peer_id: assurer,
                is_outbound: false,
            }),

            // === Assurance (outbound) ===
            Event::AssuranceSendFailed { recipient, .. } => Some(DirectedPeer {
                peer_id: recipient,
                is_outbound: true,
            }),
            Event::AssuranceSent { recipient, .. } => Some(DirectedPeer {
                peer_id: recipient,
                is_outbound: true,
            }),

            // === Assurance (inbound) ===
            Event::AssuranceReceiveFailed { sender, .. } => Some(DirectedPeer {
                peer_id: sender,
                is_outbound: false,
            }),
            Event::AssuranceReceived { sender, .. } => Some(DirectedPeer {
                peer_id: sender,
                is_outbound: false,
            }),

            // All other events are not directed (no peer ID or need EventId lookup)
            _ => None,
        }
    }

    /// Extract reason string from error events, if available.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Event::ConnectInFailed { reason, .. }
            | Event::ConnectOutFailed { reason, .. }
            | Event::Disconnected { reason, .. }
            | Event::PeerMisbehaved { reason, .. }
            | Event::AuthoringFailed { reason, .. }
            | Event::BlockVerificationFailed { reason, .. }
            | Event::BlockExecutionFailed { reason, .. }
            | Event::BlockRequestFailed { reason, .. }
            | Event::TicketGenerationFailed { reason, .. }
            | Event::WorkPackageFailed { reason, .. }
            | Event::WorkPackageSharingFailed { reason, .. }
            | Event::GuaranteeSendFailed { reason, .. }
            | Event::GuaranteeReceiveFailed { reason, .. }
            | Event::ShardRequestFailed { reason, .. }
            | Event::AssuranceSendFailed { reason, .. }
            | Event::AssuranceReceiveFailed { reason, .. } => Some(&reason.0),
            _ => None,
        }
    }

    /// Get the default travel duration for this event type (in seconds).
    /// Longer durations make motion visible at high event rates.
    pub fn travel_duration(&self) -> f32 {
        match self.event_type() {
            // Fast events (guarantees, assurances)
            EventType::SendingGuarantee
            | EventType::ReceivingGuarantee
            | EventType::AssuranceSent
            | EventType::AssuranceReceived => 2.0,

            // Medium events (shard requests, blocks)
            EventType::SendingShardRequest
            | EventType::ReceivingShardRequest
            | EventType::SendingBlockRequest
            | EventType::ReceivingBlockRequest => 2.0,

            // Slow events (connections, work package sharing)
            EventType::ConnectingOut
            | EventType::ConnectedIn
            | EventType::WorkPackageBeingShared => 3.0,

            // Default (includes WorkPackageSubmission — pulse handles visual emphasis)
            _ => 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_from_u8_valid_ranges() {
        assert_eq!(EventType::from_u8(0), Some(EventType::Dropped));
        assert_eq!(EventType::from_u8(10), Some(EventType::Status));
        assert_eq!(EventType::from_u8(13), Some(EventType::SyncStatusChanged));
        assert_eq!(EventType::from_u8(20), Some(EventType::ConnectionRefused));
        assert_eq!(EventType::from_u8(28), Some(EventType::PeerMisbehaved));
        assert_eq!(EventType::from_u8(40), Some(EventType::Authoring));
        assert_eq!(EventType::from_u8(47), Some(EventType::BlockExecuted));
        assert_eq!(EventType::from_u8(90), Some(EventType::WorkPackageSubmission));
        assert_eq!(EventType::from_u8(113), Some(EventType::GuaranteeDiscarded));
        assert_eq!(EventType::from_u8(199), Some(EventType::PreimageDiscarded));
    }

    #[test]
    fn test_event_type_from_u8_gaps() {
        assert_eq!(EventType::from_u8(1), None);
        assert_eq!(EventType::from_u8(9), None);
        assert_eq!(EventType::from_u8(14), None);
        assert_eq!(EventType::from_u8(19), None);
        assert_eq!(EventType::from_u8(29), None);
        assert_eq!(EventType::from_u8(39), None);
        assert_eq!(EventType::from_u8(48), None);
        assert_eq!(EventType::from_u8(59), None);
        assert_eq!(EventType::from_u8(69), None);
        assert_eq!(EventType::from_u8(79), None);
        assert_eq!(EventType::from_u8(85), None);
        assert_eq!(EventType::from_u8(89), None);
        assert_eq!(EventType::from_u8(114), None);
        assert_eq!(EventType::from_u8(119), None);
        assert_eq!(EventType::from_u8(200), None);
        assert_eq!(EventType::from_u8(255), None);
    }

    #[test]
    fn test_event_name() {
        assert_eq!(event_name(0), "Dropped");
        assert_eq!(event_name(10), "Status");
        assert_eq!(event_name(106), "SendingGuarantee");
        assert_eq!(event_name(199), "PreimageDiscarded");
        assert_eq!(event_name(255), "Unknown");
    }

    #[test]
    fn test_event_color_rgb() {
        assert_eq!(event_color_rgb(0), (128, 128, 128));   // Meta gray
        assert_eq!(event_color_rgb(10), (100, 200, 100));  // Status green
        assert_eq!(event_color_rgb(20), (100, 150, 255));  // Connection blue
        assert_eq!(event_color_rgb(106), (255, 100, 200));  // Guaranteeing magenta
        assert_eq!(event_color_rgb(250), (255, 255, 255)); // Unknown white
    }

    #[test]
    fn test_directed_peer_outbound() {
        let peer_id = [42u8; 32];
        let event = Event::SendingGuarantee { timestamp: 0, built_id: 1, recipient: peer_id };
        let dp = event.directed_peer().expect("should be directed");
        assert_eq!(*dp.peer_id, peer_id);
        assert!(dp.is_outbound);
    }

    #[test]
    fn test_directed_peer_inbound() {
        let peer_id = [99u8; 32];
        let event = Event::ReceivingGuarantee { timestamp: 0, sender: peer_id };
        let dp = event.directed_peer().expect("should be directed");
        assert_eq!(*dp.peer_id, peer_id);
        assert!(!dp.is_outbound);
    }

    #[test]
    fn test_directed_peer_connection_side() {
        let peer_id = [7u8; 32];
        // Local announcer = outbound
        let event = Event::BlockAnnounced { timestamp: 0, peer: peer_id, announcer: ConnectionSide::Local, slot: 1, hash: [0u8; 32] };
        let dp = event.directed_peer().unwrap();
        assert!(dp.is_outbound);

        // Remote announcer = inbound
        let event = Event::BlockAnnounced { timestamp: 0, peer: peer_id, announcer: ConnectionSide::Remote, slot: 1, hash: [0u8; 32] };
        let dp = event.directed_peer().unwrap();
        assert!(!dp.is_outbound);
    }

    #[test]
    fn test_directed_peer_none_for_non_directed() {
        let event = Event::Status { timestamp: 0, num_peers: 1, num_val_peers: 0, num_sync_peers: 0, num_guarantees: vec![], num_shards: 0, shards_size: 0, num_preimages: 0, preimages_size: 0 };
        assert!(event.directed_peer().is_none());
    }

    #[test]
    fn test_travel_duration() {
        let slow = Event::ConnectingOut { timestamp: 0, to: PeerDetails { peer_id: [0u8; 32], peer_address: PeerAddress { ipv6: [0u8; 16], port: 0 } } };
        assert_eq!(slow.travel_duration(), 3.0);

        let medium = Event::SendingGuarantee { timestamp: 0, built_id: 0, recipient: [0u8; 32] };
        assert_eq!(medium.travel_duration(), 2.0);

        // Default case
        let default_event = Event::Authored { timestamp: 0, authoring_id: 0, outline: BlockOutline { size_bytes: 0, hash: [0u8; 32], num_tickets: 0, num_preimages: 0, total_preimages_size: 0, num_guarantees: 0, num_assurances: 0, num_dispute_verdicts: 0 } };
        assert_eq!(default_event.travel_duration(), 2.0);
    }
}

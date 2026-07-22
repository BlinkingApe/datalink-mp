//! DirectPlay flag constants

use crate::DWORD;

// ============================================================================
// Session Flags (DPSESSION_*) - per Wine dplay.h lines 267-280
// ============================================================================

/// New players disabled
pub const DPSESSION_NEWPLAYERSDISABLED: DWORD = 0x00000001;
/// Players can migrate host
pub const DPSESSION_MIGRATEHOST: DWORD = 0x00000004;
/// Session has no message ID
pub const DPSESSION_NOMESSAGEID: DWORD = 0x00000008;
/// Join disabled
pub const DPSESSION_JOINDISABLED: DWORD = 0x00000020;
/// Keep alive enabled
pub const DPSESSION_KEEPALIVE: DWORD = 0x00000040;
/// No player data changes
pub const DPSESSION_NODATAMESSAGES: DWORD = 0x00000080;
/// Secure session
pub const DPSESSION_SECURESERVER: DWORD = 0x00000100;
/// Private session
pub const DPSESSION_PRIVATE: DWORD = 0x00000200;
/// Password required
pub const DPSESSION_PASSWORDREQUIRED: DWORD = 0x00000400;
/// Multicast server
pub const DPSESSION_MULTICASTSERVER: DWORD = 0x00000800;
/// Session is client/server (not peer-to-peer)
pub const DPSESSION_CLIENTSERVER: DWORD = 0x00001000;
/// Use DirectPlay protocol
pub const DPSESSION_DIRECTPLAYPROTOCOL: DWORD = 0x00002000;
/// Don't preserve message order
pub const DPSESSION_NOPRESERVEORDER: DWORD = 0x00004000;
/// Optimize for latency
pub const DPSESSION_OPTIMIZELATENCY: DWORD = 0x00008000;

// ============================================================================
// Open Flags (DPOPEN_*)
// ============================================================================

/// Join an existing session
pub const DPOPEN_JOIN: DWORD = 0x00000001;
/// Create a new session
pub const DPOPEN_CREATE: DWORD = 0x00000002;
/// Return status codes
pub const DPOPEN_RETURNSTATUS: DWORD = 0x00000080;

// ============================================================================
// EnumSessions Flags (DPENUMSESSIONS_*) - per Wine dplay.h lines 1029-1036
// ============================================================================

/// Return available sessions (joinable)
pub const DPENUMSESSIONS_AVAILABLE: DWORD = 0x00000001;
/// Return all sessions (including full/in-progress)
pub const DPENUMSESSIONS_ALL: DWORD = 0x00000002;
/// Async operation
pub const DPENUMSESSIONS_ASYNC: DWORD = 0x00000010;
/// Stop async enumeration
pub const DPENUMSESSIONS_STOPASYNC: DWORD = 0x00000020;
/// Password required sessions
pub const DPENUMSESSIONS_PASSWORDREQUIRED: DWORD = 0x00000040;
/// Return status codes
pub const DPENUMSESSIONS_RETURNSTATUS: DWORD = 0x00000080;

/// Enumeration timed out (passed in final callback)
pub const DPESC_TIMEDOUT: DWORD = 0x00000001;

// ============================================================================
// EnumPlayers Flags (DPENUMPLAYERS_*)
// ============================================================================

/// All players
pub const DPENUMPLAYERS_ALL: DWORD = 0x00000000;
/// Local players only
pub const DPENUMPLAYERS_LOCAL: DWORD = 0x00000008;
/// Remote players only
pub const DPENUMPLAYERS_REMOTE: DWORD = 0x00000010;
/// Players in a group
pub const DPENUMPLAYERS_GROUP: DWORD = 0x00000020;
/// Server player only
pub const DPENUMPLAYERS_SERVERPLAYER: DWORD = 0x00000100;
/// Spectator players
pub const DPENUMPLAYERS_SPECTATOR: DWORD = 0x00000200;
/// Owner of a group
pub const DPENUMPLAYERS_OWNER: DWORD = 0x02000000;

// ============================================================================
// EnumGroups Flags (DPENUMGROUPS_*)
// ============================================================================

/// All groups
pub const DPENUMGROUPS_ALL: DWORD = 0x00000000;
/// Local groups only
pub const DPENUMGROUPS_LOCAL: DWORD = 0x00000008;
/// Remote groups only
pub const DPENUMGROUPS_REMOTE: DWORD = 0x00000010;
/// Hidden groups
pub const DPENUMGROUPS_HIDDEN: DWORD = 0x00000040;
/// Staging area groups
pub const DPENUMGROUPS_STAGINGAREA: DWORD = 0x00000080;
/// Shortcut groups
pub const DPENUMGROUPS_SHORTCUT: DWORD = 0x00000100;

// ============================================================================
// Create Player Flags (DPPLAYER_*)
// ============================================================================

/// Local player
pub const DPPLAYER_LOCAL: DWORD = 0x00000008;
/// Server player (host)
pub const DPPLAYER_SERVERPLAYER: DWORD = 0x00000100;
/// Spectator player
pub const DPPLAYER_SPECTATOR: DWORD = 0x00000200;
/// Player owns a group
pub const DPPLAYER_OWNER: DWORD = 0x02000000;

// ============================================================================
// Create Group Flags (DPGROUP_*)
// ============================================================================

/// Local group
pub const DPGROUP_LOCAL: DWORD = 0x00000008;
/// Hidden group
pub const DPGROUP_HIDDEN: DWORD = 0x00000040;
/// Staging area group
pub const DPGROUP_STAGINGAREA: DWORD = 0x00000080;
/// Shortcut group
pub const DPGROUP_SHORTCUT: DWORD = 0x00000100;

// ============================================================================
// Send Flags (DPSEND_*)
// ============================================================================

/// Default send (non-guaranteed)
pub const DPSEND_NONGUARANTEED: DWORD = 0x00000000;
/// Guaranteed delivery
pub const DPSEND_GUARANTEED: DWORD = 0x00000001;
/// High priority
pub const DPSEND_HIGHPRIORITY: DWORD = 0x00000002;
/// Open stream
pub const DPSEND_OPENSTREAM: DWORD = 0x00000008;
/// Close stream
pub const DPSEND_CLOSESTREAM: DWORD = 0x00000010;
/// Signed message
pub const DPSEND_SIGNED: DWORD = 0x00000020;
/// Encrypted message
pub const DPSEND_ENCRYPTED: DWORD = 0x00000040;
/// Async send (don't block)
pub const DPSEND_ASYNC: DWORD = 0x00000200;
/// Don't bundle with other sends
pub const DPSEND_NOSENDCOMPLETEMSG: DWORD = 0x00000400;
/// Max priority
pub const DPSEND_MAX_PRI: DWORD = 0x0000FFFF;
/// Max priority high word
pub const DPSEND_MAX_PRIORITY: DWORD = 0xFFFF;

// ============================================================================
// Receive Flags (DPRECEIVE_*)
// ============================================================================

/// Receive all messages
pub const DPRECEIVE_ALL: DWORD = 0x00000001;
/// Return top of queue
pub const DPRECEIVE_TOPLAYER: DWORD = 0x00000002;
/// Return from specific player
pub const DPRECEIVE_FROMPLAYER: DWORD = 0x00000004;
/// Peek at message (don't remove)
pub const DPRECEIVE_PEEK: DWORD = 0x00000008;

// ============================================================================
// GetCaps Flags (DPGETCAPS_*)
// ============================================================================

/// Get guaranteed caps
pub const DPGETCAPS_GUARANTEED: DWORD = 0x00000001;

// ============================================================================
// SetData/GetData Flags (DPSET_*/DPGET_*)
// ============================================================================

/// Remote data
pub const DPSET_REMOTE: DWORD = 0x00000000;
/// Local data
pub const DPSET_LOCAL: DWORD = 0x00000001;
/// Guaranteed update
pub const DPSET_GUARANTEED: DWORD = 0x00000002;

/// Remote data
pub const DPGET_REMOTE: DWORD = 0x00000000;
/// Local data
pub const DPGET_LOCAL: DWORD = 0x00000001;

// ============================================================================
// Connection Flags (DPCONNECTION_*)
// ============================================================================

/// DirectPlay connection
pub const DPCONNECTION_DIRECTPLAY: DWORD = 0x00000001;
/// DirectPlayLobby connection
pub const DPCONNECTION_DIRECTPLAYLOBBY: DWORD = 0x00000002;

// ============================================================================
// EnumConnections Flags (DPENUMCONNECTIONS_*)
// ============================================================================

/// DirectPlay connections
pub const DPENUMCONNECTIONS_DIRECTPLAY: DWORD = 0x00000001;
/// DirectPlayLobby connections
pub const DPENUMCONNECTIONS_DIRECTPLAYLOBBY: DWORD = 0x00000002;
/// Return status
pub const DPENUMCONNECTIONS_RETURNSTATUS: DWORD = 0x00000080;

// ============================================================================
// Capability Flags (DPCAPS_*) - per Wine dplay.h lines 1081-1093
// ============================================================================

/// Session is hosted locally
pub const DPCAPS_ISHOST: DWORD = 0x00000002;
/// Group sends are optimized
pub const DPCAPS_GROUPOPTIMIZED: DWORD = 0x00000008;
/// Keepalives are optimized
pub const DPCAPS_KEEPALIVEOPTIMIZED: DWORD = 0x00000010;
/// Guaranteed delivery is optimized
pub const DPCAPS_GUARANTEEDOPTIMIZED: DWORD = 0x00000020;
/// Guaranteed delivery is supported
pub const DPCAPS_GUARANTEEDSUPPORTED: DWORD = 0x00000040;
/// Message signing is supported
pub const DPCAPS_SIGNINGSUPPORTED: DWORD = 0x00000080;
/// Message encryption is supported
pub const DPCAPS_ENCRYPTIONSUPPORTED: DWORD = 0x00000100;
/// Async cancel supported
pub const DPCAPS_ASYNCCANCELSUPPORTED: DWORD = 0x00001000;
/// Async cancel all supported
pub const DPCAPS_ASYNCCANCELALLSUPPORTED: DWORD = 0x00002000;
/// Send timeout is supported
pub const DPCAPS_SENDTIMEOUTSUPPORTED: DWORD = 0x00004000;
/// Send priority is supported
pub const DPCAPS_SENDPRIORITYSUPPORTED: DWORD = 0x00008000;
/// Async operations are supported
pub const DPCAPS_ASYNCSUPPORTED: DWORD = 0x00010000;

// ============================================================================
// Player Type (for system messages)
// ============================================================================

/// Type is a player
pub const CYCLOPSDPPLAYERTYPE_PLAYER: DWORD = 0x00010000;
/// Type is a group
pub const CYCLOPSDPPLAYERTYPE_GROUP: DWORD = 0x00020000;

// ============================================================================
// Message Queue Flags
// ============================================================================

/// Send queue
pub const DPMESSAGEQUEUE_SEND: DWORD = 0x00000001;
/// Receive queue
pub const DPMESSAGEQUEUE_RECEIVE: DWORD = 0x00000002;

// ============================================================================
// Cancel Flags
// ============================================================================

/// Cancel all pending sends
pub const CYCLOPSDPCANCEL_ALL: DWORD = 0x00000001;
/// Cancel priority range
pub const DPCANCEL_PRIORITY: DWORD = 0x00000002;

// ============================================================================
// Lobby Message Flags
// ============================================================================

/// Standard lobby message
pub const CYCLOPSDPLMSG_STANDARD: DWORD = 0x00000001;
/// System lobby message
pub const CYCLOPSDPLMSG_SYSTEM: DWORD = 0x00000002;
/// Lobby sends lobby message
pub const CYCLOPSDPLMSG_SENDLOBBYMESSAGE: DWORD = 0x00000004;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_values() {
        // Verify some known flag values match Windows SDK
        assert_eq!(DPOPEN_JOIN, 0x00000001);
        assert_eq!(DPOPEN_CREATE, 0x00000002);
        assert_eq!(DPSEND_GUARANTEED, 0x00000001);
    }
}

use bytes::Bytes;
use serde::Serialize;

use crate::types::states::JobState;
use crate::types::tube::TubeStats;

/// A command sent by the client to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Places a job onto the currently `use`d queue.
    ///
    /// On the wire: `put <pri> <delay> <ttr>`
    Put {
        pri: u32,
        delay: u32,
        ttr: u32,
        n_bytes: u32,
    },
    /// Awaits a job from all the `watch`ed queues, blocking until one appears
    /// (or until the server shuts down).
    ///
    /// On the wire: `reserve`
    Reserve,
    /// As `reserve`, but after `timeout` seconds pass, a `TIMED_OUT` response
    /// is sent instead.
    ///
    /// On the wire: `reserve-with-timeout <seconds>`
    ReserveWithTimeout { timeout: u32 },
    /// Reserves a job with a given ID if it exists and is not already reserved,
    /// otherwise returning `NOT_FOUND`.
    ///
    /// On the wire: `reserve-job <id>`
    ReserveJob { id: u64 },
    /// Releases a job reserved by the same client, returning it to the ready
    /// queue. Returns `RELEASED` or `NOT_FOUND` in most cases, but can also
    /// return `BURIED` if the server was unable to expand the priority queue
    /// data structure.
    ///
    /// On the wire: `release <id> <pri> <delay>`
    Release { id: u64, pri: u32, delay: u32 },
    /// Deletes a job reserved by the same client, or in the ready, buried, or
    /// delayed states. Returns `DELETED` or `NOT_FOUND`.
    ///
    /// On the wire: `delete <id>`
    Delete { id: u64 },
    /// Buries a job reserved by the same client. Returns `BURIED` or
    /// `NOT_FOUND`.
    ///
    /// On the wire: `bury <id>`
    Bury { id: u64, pri: u32 },
    /// Refreshes the Time To Run (TTR) of a job reserved by the same client.
    /// Returns `TOUCHED` or `NOT_FOUND`.
    ///
    /// On the wire: `touch <id>`
    Touch { id: u64 },
    /// Adds a tube to the watchlist for this client. Always replies with
    /// `WATCHING <number of watched tubes>`.
    ///
    /// On the wire: `watch <tube>`
    Watch { tube: Vec<u8> },
    /// Reverses the effect of `watch` on this client. Returns `WATCHING <n>` or
    /// `NOT_IGNORED` if this would remove the last queue in the watchlist.
    ///
    /// On the wire: `ignore <tube>`
    Ignore { tube: Vec<u8> },
    /// Returns the data for the job with this ID, regardless of its state.
    /// Response is either `FOUND <id> <bytes>` or `NOT_FOUND`, in common with
    /// all requests in the `peek` family.
    ///
    /// On the wire: `peek <id>`
    Peek { id: u64 },
    /// Returns the data for the next ready job on the currently-used tube.
    ///
    /// On the wire: `peek-ready`
    PeekReady,
    /// Returns the data for the next delayed job that will become ready on the
    /// currently-used tube.
    ///
    /// On the wire: `peek-delayed`
    PeekDelayed,
    /// Returns the data for the first available buried job on the currently-
    /// used tube.
    /// TODO: is this FIFO or FILO?
    ///
    /// On the wire: `peek-buried`
    PeekBuried,
    /// Promotes up to `bound` jobs on the currently-used tube from buried to
    /// the ready states, returning `KICKED <count>` with the actual number of
    /// jobs kicked. If no buried jobs exist, it promotes delayed jobs instead.
    /// In other words, if at least one buried jobs exist, at least two kick
    /// commands must be executed for any delayed jobs to be kicked.
    ///
    /// On the wire: `kick <bound>
    Kick { bound: u64 },
    /// Promotes a single job from buried or delayed to ready by its ID.
    /// Returns `KICKED` if successful, otherwise `NOT_FOUND` if the job ID
    /// doesn't exist or the job is not kickable.
    ///
    /// On the wire: `kick-job <id>`
    KickJob { id: u64 },
    /// Provides information about the job with the given ID, including which
    /// tube it's on, state, priority, timings, and the number of state
    /// transitions it's undergone.
    ///
    /// As with all responses from the `Stats` and `ListTubes` families of
    /// commands, returns an `OK <n_bytes>` response with associated data.
    ///
    /// As with all responses from the `Stats` family of commands, returns a
    /// YAML object.
    ///
    /// On the wire: `stats-job <id>`
    StatsJob { id: u64 },
    /// Returns information about a tube, including the number of jobs in each
    /// state, number of active consumers and producers, total jobs handled, and
    /// pause status.
    ///
    /// On the wire: `stats <tube>`
    StatsTube { tube: Vec<u8> },
    /// Exposes information about the server, including global job counts by
    /// state, number of each command executed, and various internal statuses.
    ///
    /// On the wire: `stats`
    StatsServer,
    /// Returns a list of which tubes currently exist (have been `use`d by any
    /// consumer).
    ///
    /// As for all commands in the `ListTubes` family, returns an `OK <n_bytes>`
    /// response with associated data encoding a YAML-format list.
    ///
    /// On the wire: `list-tubes`
    ListTubes,
    /// Returns the tube name this client is currently using as `USING <tube>`.
    ///
    /// On the wire: `list-tube-used`
    ListTubeUsed,
    /// Returns any tubes this client is currently watching.
    ///
    /// On the wire: `list-tubes-watched`
    ListTubesWatched,
    /// Requests that the server close this connection, releasing any
    /// server-side resources in doing so.
    ///
    /// On the wire: `quit`
    Quit,
    /// Pause a tube for a given period, preventing new jobs being reserved for
    /// `delay` seconds. Returns `PAUSED` or `NOT_FOUND`.
    ///
    /// On the wire: `pause-tube <tube> <delay>`
    PauseTube { tube: Vec<u8>, delay: u32 },
    /// On the wire: `use <tube>`
    Use { tube: Vec<u8> },
}

/// All possible response types to a `BeanstalkRequest`.
#[derive(Debug, PartialEq)]
pub enum Response {
    /// Indicates the server cannot handle a job due to memory pressure. Can be
    /// sent in response to any command.
    ///
    /// On the wire: `OUT_OF_MEMORY`.
    OutOfMemory,
    /// Indicates a server bug. Can be sent in response to any command.
    ///
    /// On the wire: `INTERNAL_ERROR`.
    InternalError,
    /// The client sent a bad request, typically because:
    ///
    /// * The request exceeded 224 bytes , including trailing CRLF.
    /// * A tube name exceeded 200 bytes or was invalid.
    /// * A non-number was provided where a number was expected, or the number
    ///   was out of range.
    ///
    /// On the wire: `BAD_FORMAT`.
    BadFormat,
    /// The client sent a bad request with an unrecognised command.
    ///
    /// On the wire: `UNKNOWN_COMMAND`.
    UnknownCommand,
    /// In response to a `put`, indicates a job was created with the given ID.
    ///
    /// On the wire: `INSERTED <id>`.
    Inserted { id: u64 },
    /// In response to a `put`, indicates the job couldn't be handled due to
    /// memory pressure and so was immediately buried.
    ///
    /// On the wire: `BURIED <id>`.
    BuriedID { id: u64 },
    /// In response to a `put`, indicates the job data was not terminated by a
    /// CRLF sequence.
    ///
    /// On the wire: `EXPECTED_CRLF`.
    ExpectedCRLF,
    /// In response to a `put`, indicates the job body was larger than what the
    /// server is configured to accept.
    ///
    /// On the wire: `JOB_TOO_BIG`.
    JobTooBig,
    /// In response to a `put`, indicates the server is not currently accepting
    /// jobs.
    ///
    /// On the wire: `DRAINING`.
    Draining,
    /// In response to a `use` or `list-tube-used`, indicates the client is
    /// watching this tube.
    ///
    /// On the wire: `USING <tube>`.
    Using { tube: Vec<u8> },
    /// In response to a `reserve` or `reserve-with-timeout`, indicates the
    /// client has reserved a job that will exceed its Time To Run (TTR) in the
    /// next second and so will be released automatically. Can be returned
    /// immediately or after a delay.
    ///
    /// On the wire: `DEADLINE_SOON`.
    DeadlineSoon,
    /// In response to a `reserve-with-timeout`, indicates the timeout provided
    /// expired with no job becoming available.
    ///
    /// On the wire: `TIMED_OUT`.
    TimedOut,
    /// In response to a `reserve`, `reserve-with-timeout`, or `reserve-job`,
    /// provides the ID and of the job that was just reserved.
    ///
    /// On the wire: `RESERVED <id> <n_bytes>`.
    Reserved { id: u64 },
    /// In response to a `peek`-family command, indicates success.
    ///
    /// On the wire: `FOUND <id> <n_bytes>`.
    Found { id: u64 },
    /// After a Reserved or Found message, a chunk of the job data
    JobChunk(Bytes),
    /// Ends a job
    JobEnd,
    /// In response to any of the following commands, indicates a general state
    /// where a specific job isn't known to the server, or doesn't satisfy
    /// a precondition to be returned by the command.
    ///
    /// Specific cases include:
    ///
    /// * `reserve-job`: the job is reserved or unknown.
    /// * `delete`: the job is unknown; or the job isn't ready, buried, or
    ///   reserved by this client.
    /// * `release`, `bury`, or `touch`: the job is unknown or is not reserved
    ///   by this client.
    /// * `peek`: the job is unknown.
    /// * `peek-*` family: no such jobs exist on the currently `use`d tube.
    /// * `kick-job`: the job is unknown or is neither buried nor delayed, or
    ///   allowable if an internal server error occurred preventing the kick.
    /// * `pause-tube`: the tube does not exist.
    ///
    /// On the wire: `NOT_FOUND`.
    NotFound,
    /// In response to a `delete` command, indicates the job was successfully
    /// deleted.
    ///
    /// On the wire: `DELETED`.
    Deleted,
    /// In response to a `release` command, indicates the job was successfully
    /// released back to the ready or delayed states.
    ///
    /// On the wire: `RELEASED`.
    Released,
    /// In response to a `release`, indicates the job couldn't be handled due to
    /// memory pressure and so was immediately buried.
    ///
    /// In response to a `bury`, indicates success.
    ///
    /// On the wire: `BURIED`.
    Buried,
    /// In response to a `touch`, indicates the job's TTR was refreshed.
    ///
    /// On the wire: `TOUCHED`.
    Touched,
    /// In response to a `watch` or `ignore`, indicates success and the number
    /// of tubes currently watched by the client.
    ///
    /// On the wire: `WATCHING <count>`.
    Watching { count: u32 },
    /// In response to an `ignore`, indicates the command failed as it would
    /// leave the client with an empty watchlist.
    ///
    /// On the wire: `NOT_IGNORED`.
    NotIgnored,
    /// In response to a `kick`, indicates success with the number of jobs
    /// kicked from the buried xor delayed states.
    ///
    /// On the wire: `KICKED <count>`.
    KickedCount { count: u64 },
    /// In response to a `kick-job`, indicates success.
    ///
    /// On the wire: `KICKED`.
    Kicked,
    /// In response to a `stats-job`, indicates success.
    ///
    /// On the wire: `OK <n_bytes>` plus data in YAML dictionary format.
    OkStatsJob { data: JobStats },
    ///In response to a `stats`, indicates success.
    ///
    /// On the wire: `OK <n_bytes>` plus data in YAML dictionary format.
    OkStats { data: ServerStats },
    ///In response to a `stats-tube`, indicates success.
    ///
    /// On the wire: `OK <n_bytes>` plus data in YAML dictionary format.
    OkStatsTube { data: TubeStatsResp },
    ///In response to a `list-tubes` or `list-tubes-watched`, indicates success.
    ///
    /// On the wire: `OK <n_bytes>` plus data in YAML *list* format.
    OkListTubes { tubes: Vec<Vec<u8>> },
    /// In response to a `pause-tube`, indicates success.
    ///
    /// On the wire: `PAUSED`.
    Paused,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct JobStats {
    /// job ID
    id: u64,
    /// tube containing job
    tube: Vec<u8>,
    /// job state
    state: JobState,
    /// priority set by last put/release/bury
    pri: u32,

    /// time in seconds since creation
    age: u32, // TODO: size
    /// seconds remaining until ready
    delay: u32, // TODO: size
    /// allowed processing time in seconds
    ttr: u32, // TODO: size
    /// time until job returns to ready queue
    #[serde(rename = "time-left")]
    time_left: u32, // TODO: size

    /// earliest binlog file containing job
    file: u32, // TODO: size

    /// number of times job reserved
    reserves: u64, // TODO: size
    /// number of times job timed out
    timeouts: u64, // TODO: size
    /// number of times job released
    releases: u64, // TODO: size
    /// number of times job buried
    buries: u64, // TODO: size
    /// number of times job kicked
    kicks: u64, // TODO: size
}

#[derive(Debug, PartialEq, Serialize)]
pub struct TubeStatsResp {
    /// tube name
    name: Vec<u8>,
    #[serde(flatten)]
    ts: TubeStats,
    /// seconds remaining until the queue is un-paused.
    #[serde(rename = "pause-time-left")]
    pause_time_left: u32,
}

// TODO: decompose into component structs
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct ServerStats {
    /// number of ready jobs with priority < 1024
    #[serde(rename = "current-jobs-urgent")]
    current_jobs_urgent: u64,
    /// number of jobs in the ready queue
    #[serde(rename = "current-jobs-ready")]
    current_jobs_ready: u64,
    /// number of jobs reserved by all clients
    #[serde(rename = "current-jobs-reserved")]
    current_jobs_reserved: u64,
    /// number of delayed jobs
    #[serde(rename = "current-jobs-delayed")]
    current_jobs_delayed: u64,
    /// number of buried jobs
    #[serde(rename = "current-jobs-buried")]
    current_jobs_buried: u64,

    /// number of X commands
    #[serde(rename = "cmd-put")]
    cmd_put: u64,
    /// number of X commands
    #[serde(rename = "cmd-peek")]
    cmd_peek: u64,
    /// number of X commands
    #[serde(rename = "cmd-peek-ready")]
    cmd_peek_ready: u64,
    /// number of X commands
    #[serde(rename = "cmd-peek-delayed")]
    cmd_peek_delayed: u64,
    /// number of X commands
    #[serde(rename = "cmd-peek-buried")]
    cmd_peek_buried: u64,
    /// number of X commands
    #[serde(rename = "cmd-reserve")]
    cmd_reserve: u64,
    /// number of X commands
    #[serde(rename = "cmd-reserve-with-timeout")]
    cmd_reserve_with_timeout: u64,
    /// number of X commands
    #[serde(rename = "cmd-touch")]
    cmd_touch: u64,
    /// number of X commands
    #[serde(rename = "cmd-use")]
    cmd_use: u64,
    /// number of X commands
    #[serde(rename = "cmd-watch")]
    cmd_watch: u64,
    /// number of X commands
    #[serde(rename = "cmd-ignore")]
    cmd_ignore: u64,
    /// number of X commands
    #[serde(rename = "cmd-delete")]
    cmd_delete: u64,
    /// number of X commands
    #[serde(rename = "cmd-release")]
    cmd_release: u64,
    /// number of X commands
    #[serde(rename = "cmd-bury")]
    cmd_bury: u64,
    /// number of X commands
    #[serde(rename = "cmd-kick")]
    cmd_kick: u64,
    /// number of X commands
    #[serde(rename = "cmd-stats")]
    cmd_stats: u64,
    /// number of X commands
    #[serde(rename = "cmd-stats-job")]
    cmd_stats_job: u64,
    /// number of X commands
    #[serde(rename = "cmd-stats-tube")]
    cmd_stats_tube: u64,
    /// number of X commands
    #[serde(rename = "cmd-list-tubes")]
    cmd_list_tubes: u64,
    /// number of X commands
    #[serde(rename = "cmd-list-tube-used")]
    cmd_list_tube_used: u64,
    /// number of X commands
    #[serde(rename = "cmd-list-tubes-watched")]
    cmd_list_tubes_watched: u64,
    /// number of X commands
    #[serde(rename = "cmd-pause-tube")]
    cmd_pause_tube: u64,

    /// cumulative count of times a job has timed out
    #[serde(rename = "job-timeouts")]
    job_timeouts: u64,
    /// cumulative count of jobs created
    #[serde(rename = "total-jobs")]
    total_jobs: u64,
    /// maximum number of bytes in a job
    #[serde(rename = "max-job-size")]
    max_job_size: u64,
    /// number of currently-existing tubes
    #[serde(rename = "current-tubes")]
    current_tubes: u64,
    /// number of currently open connections
    #[serde(rename = "current-connections")]
    current_connections: u64,
    /// number of open connections that have each issued at least one put command
    #[serde(rename = "current-producers")]
    current_producers: u64,
    /// number of open connections that have each issued at least one reserve command
    #[serde(rename = "current-workers")]
    current_workers: u64,
    /// number of open connections that have issued a reserve command but not yet received a response
    #[serde(rename = "current-waiting")]
    current_waiting: u64,
    /// cumulative count of connections
    #[serde(rename = "total-connections")]
    total_connections: u64,
    /// process id of the server
    pid: u32,
    /// version string of the server
    version: &'static str,
    /// cumulative user CPU time of this process in seconds and microseconds
    #[serde(rename = "rusage-utime")]
    rusage_utime: u64,
    /// cumulative system CPU time of this process in seconds and microseconds
    #[serde(rename = "rusage-stime")]
    rusage_stime: u64,
    /// number of seconds since this server process started running
    uptime: u32,

    /// index of the oldest binlog file needed to store the current jobs
    #[serde(rename = "binlog-oldest-index")]
    binlog_oldest_index: u64,
    /// index of the current binlog file being written to. If binlog is not active this value will be 0
    #[serde(rename = "binlog-current-index")]
    binlog_current_index: u64,
    /// maximum size in bytes a binlog file is allowed to get before a new binlog file is opened
    #[serde(rename = "binlog-max-size")]
    binlog_max_size: u64,
    /// cumulative number of records written to the binlog
    #[serde(rename = "binlog-records-written")]
    binlog_records_written: u64,
    /// cumulative number of records written as part of compaction
    #[serde(rename = "binlog-records-migrated")]
    binlog_records_migrated: u64,

    /// is server is in drain mode
    draining: bool,
    /// random id string for this server process, generated every time the
    /// process starts
    id: Vec<u8>,
    // hostname of the machine as determined by uname
    hostname: Vec<u8>,
    /// OS version as determined by uname
    os: Vec<u8>,
    /// machine architecture as determined by uname
    platform: Vec<u8>,
}

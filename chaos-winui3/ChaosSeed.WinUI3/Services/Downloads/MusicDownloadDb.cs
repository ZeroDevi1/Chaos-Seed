using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using Microsoft.Data.Sqlite;

namespace ChaosSeed.WinUI3.Services.Downloads;

public sealed record DownloadSessionRow
{
    public string SessionId { get; init; } = "";
    public long StartedAtUnixMs { get; init; }
    public long LastUpdateUnixMs { get; init; }
    public bool Done { get; init; }

    public string TargetType { get; init; } = "";
    public string? Service { get; init; }
    public string? Title { get; init; }
    public string? Artist { get; init; }
    public string? Album { get; init; }
    public string? CoverUrl { get; init; }

    public string OutDir { get; init; } = "";
    public string QualityId { get; init; } = "";
    public string? PathTemplate { get; init; }
    public bool Overwrite { get; init; }
    public int Concurrency { get; init; }
    public int Retries { get; init; }

    public int Total { get; init; }
    public int DoneCount { get; init; }
    public int Failed { get; init; }
    public int Skipped { get; init; }
    public int Canceled { get; init; }
}

public sealed record DownloadJobRow
{
    public string SessionId { get; init; } = "";
    public int JobIndex { get; init; }

    public string? TrackId { get; init; }
    public string? TrackTitle { get; init; }
    public string? TrackArtists { get; init; }
    public string? TrackAlbum { get; init; }

    public string State { get; init; } = "";
    public string? Path { get; init; }
    public long? Bytes { get; init; }
    public string? Error { get; init; }
}

public sealed class MusicDownloadDb
{
    private readonly string _dbPath;
    private readonly object _gate = new();

    public MusicDownloadDb(string dbPath)
    {
        _dbPath = dbPath ?? throw new ArgumentNullException(nameof(dbPath));
        Initialize();
    }

    public static string GetDefaultDbPath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(root, "ChaosSeed.WinUI3", "music-downloads.v1.sqlite3");
    }

    private string ConnectionString => new SqliteConnectionStringBuilder
    {
        DataSource = _dbPath,
        Mode = SqliteOpenMode.ReadWriteCreate,
        Cache = SqliteCacheMode.Shared,
    }.ToString();

    private void Initialize()
    {
        lock (_gate)
        {
            var dir = Path.GetDirectoryName(_dbPath);
            if (!string.IsNullOrWhiteSpace(dir))
            {
                Directory.CreateDirectory(dir);
            }

            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();

            using (var cmd = conn.CreateCommand())
            {
                cmd.CommandText = "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA user_version;";
                _ = cmd.ExecuteScalar();
            }

            using (var cmd = conn.CreateCommand())
            {
                cmd.CommandText = @"
CREATE TABLE IF NOT EXISTS download_session (
  session_id TEXT PRIMARY KEY,
  started_at_unix_ms INTEGER NOT NULL,
  last_update_unix_ms INTEGER NOT NULL,
  done INTEGER NOT NULL,
  target_type TEXT NOT NULL,
  service TEXT NULL,
  title TEXT NULL,
  artist TEXT NULL,
  album TEXT NULL,
  cover_url TEXT NULL,
  out_dir TEXT NOT NULL,
  quality_id TEXT NOT NULL,
  path_template TEXT NULL,
  overwrite INTEGER NOT NULL,
  concurrency INTEGER NOT NULL,
  retries INTEGER NOT NULL,
  total INTEGER NOT NULL DEFAULT 0,
  done_count INTEGER NOT NULL DEFAULT 0,
  failed INTEGER NOT NULL DEFAULT 0,
  skipped INTEGER NOT NULL DEFAULT 0,
  canceled INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS download_job (
  session_id TEXT NOT NULL,
  job_index INTEGER NOT NULL,
  track_id TEXT NULL,
  track_title TEXT NULL,
  track_artists TEXT NULL,
  track_album TEXT NULL,
  state TEXT NOT NULL,
  path TEXT NULL,
  bytes INTEGER NULL,
  error TEXT NULL,
  PRIMARY KEY(session_id, job_index),
  FOREIGN KEY(session_id) REFERENCES download_session(session_id) ON DELETE CASCADE
);";
                cmd.ExecuteNonQuery();
            }
        }
    }

    public void UpsertSession(DownloadSessionRow row)
    {
        lock (_gate)
        {
            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();
            using var cmd = conn.CreateCommand();
            cmd.CommandText = @"
INSERT INTO download_session (
  session_id, started_at_unix_ms, last_update_unix_ms, done,
  target_type, service, title, artist, album, cover_url,
  out_dir, quality_id, path_template, overwrite, concurrency, retries,
  total, done_count, failed, skipped, canceled
) VALUES (
  $session_id, $started_at_unix_ms, $last_update_unix_ms, $done,
  $target_type, $service, $title, $artist, $album, $cover_url,
  $out_dir, $quality_id, $path_template, $overwrite, $concurrency, $retries,
  $total, $done_count, $failed, $skipped, $canceled
)
ON CONFLICT(session_id) DO UPDATE SET
  last_update_unix_ms=excluded.last_update_unix_ms,
  done=excluded.done,
  target_type=excluded.target_type,
  service=excluded.service,
  title=excluded.title,
  artist=excluded.artist,
  album=excluded.album,
  cover_url=excluded.cover_url,
  out_dir=excluded.out_dir,
  quality_id=excluded.quality_id,
  path_template=excluded.path_template,
  overwrite=excluded.overwrite,
  concurrency=excluded.concurrency,
  retries=excluded.retries,
  total=excluded.total,
  done_count=excluded.done_count,
  failed=excluded.failed,
  skipped=excluded.skipped,
  canceled=excluded.canceled;";

            cmd.Parameters.AddWithValue("$session_id", row.SessionId);
            cmd.Parameters.AddWithValue("$started_at_unix_ms", row.StartedAtUnixMs);
            cmd.Parameters.AddWithValue("$last_update_unix_ms", row.LastUpdateUnixMs);
            cmd.Parameters.AddWithValue("$done", row.Done ? 1 : 0);
            cmd.Parameters.AddWithValue("$target_type", row.TargetType);
            cmd.Parameters.AddWithValue("$service", (object?)row.Service ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$title", (object?)row.Title ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$artist", (object?)row.Artist ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$album", (object?)row.Album ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$cover_url", (object?)row.CoverUrl ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$out_dir", row.OutDir);
            cmd.Parameters.AddWithValue("$quality_id", row.QualityId);
            cmd.Parameters.AddWithValue("$path_template", (object?)row.PathTemplate ?? DBNull.Value);
            cmd.Parameters.AddWithValue("$overwrite", row.Overwrite ? 1 : 0);
            cmd.Parameters.AddWithValue("$concurrency", row.Concurrency);
            cmd.Parameters.AddWithValue("$retries", row.Retries);
            cmd.Parameters.AddWithValue("$total", row.Total);
            cmd.Parameters.AddWithValue("$done_count", row.DoneCount);
            cmd.Parameters.AddWithValue("$failed", row.Failed);
            cmd.Parameters.AddWithValue("$skipped", row.Skipped);
            cmd.Parameters.AddWithValue("$canceled", row.Canceled);

            cmd.ExecuteNonQuery();
        }
    }

    public void UpsertJobs(string sessionId, IEnumerable<DownloadJobRow> jobs)
    {
        lock (_gate)
        {
            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();
            using var tx = conn.BeginTransaction();

            foreach (var j in jobs)
            {
                using var cmd = conn.CreateCommand();
                cmd.Transaction = tx;
                cmd.CommandText = @"
INSERT INTO download_job (
  session_id, job_index,
  track_id, track_title, track_artists, track_album,
  state, path, bytes, error
) VALUES (
  $session_id, $job_index,
  $track_id, $track_title, $track_artists, $track_album,
  $state, $path, $bytes, $error
)
ON CONFLICT(session_id, job_index) DO UPDATE SET
  track_id=COALESCE(excluded.track_id, download_job.track_id),
  track_title=COALESCE(excluded.track_title, download_job.track_title),
  track_artists=COALESCE(excluded.track_artists, download_job.track_artists),
  track_album=COALESCE(excluded.track_album, download_job.track_album),
  state=excluded.state,
  path=excluded.path,
  bytes=excluded.bytes,
  error=excluded.error;";

                cmd.Parameters.AddWithValue("$session_id", sessionId);
                cmd.Parameters.AddWithValue("$job_index", j.JobIndex);
                cmd.Parameters.AddWithValue("$track_id", (object?)j.TrackId ?? DBNull.Value);
                cmd.Parameters.AddWithValue("$track_title", (object?)j.TrackTitle ?? DBNull.Value);
                cmd.Parameters.AddWithValue("$track_artists", (object?)j.TrackArtists ?? DBNull.Value);
                cmd.Parameters.AddWithValue("$track_album", (object?)j.TrackAlbum ?? DBNull.Value);
                cmd.Parameters.AddWithValue("$state", j.State ?? "");
                cmd.Parameters.AddWithValue("$path", (object?)j.Path ?? DBNull.Value);
                cmd.Parameters.AddWithValue("$bytes", j.Bytes is null ? DBNull.Value : j.Bytes.Value);
                cmd.Parameters.AddWithValue("$error", (object?)j.Error ?? DBNull.Value);

                cmd.ExecuteNonQuery();
            }

            tx.Commit();
        }
    }

    public List<DownloadSessionRow> ListSessions(int limit = 200)
    {
        lock (_gate)
        {
            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();
            using var cmd = conn.CreateCommand();
            cmd.CommandText = @"
SELECT
  session_id, started_at_unix_ms, last_update_unix_ms, done,
  target_type, service, title, artist, album, cover_url,
  out_dir, quality_id, path_template, overwrite, concurrency, retries,
  total, done_count, failed, skipped, canceled
FROM download_session
ORDER BY started_at_unix_ms DESC
LIMIT $limit;";
            cmd.Parameters.AddWithValue("$limit", Math.Clamp(limit, 1, 2000));

            using var r = cmd.ExecuteReader();
            var outList = new List<DownloadSessionRow>();
            while (r.Read())
            {
                outList.Add(new DownloadSessionRow
                {
                    SessionId = r.GetString(0),
                    StartedAtUnixMs = r.GetInt64(1),
                    LastUpdateUnixMs = r.GetInt64(2),
                    Done = r.GetInt64(3) != 0,
                    TargetType = r.GetString(4),
                    Service = r.IsDBNull(5) ? null : r.GetString(5),
                    Title = r.IsDBNull(6) ? null : r.GetString(6),
                    Artist = r.IsDBNull(7) ? null : r.GetString(7),
                    Album = r.IsDBNull(8) ? null : r.GetString(8),
                    CoverUrl = r.IsDBNull(9) ? null : r.GetString(9),
                    OutDir = r.GetString(10),
                    QualityId = r.GetString(11),
                    PathTemplate = r.IsDBNull(12) ? null : r.GetString(12),
                    Overwrite = r.GetInt64(13) != 0,
                    Concurrency = r.GetInt32(14),
                    Retries = r.GetInt32(15),
                    Total = r.GetInt32(16),
                    DoneCount = r.GetInt32(17),
                    Failed = r.GetInt32(18),
                    Skipped = r.GetInt32(19),
                    Canceled = r.GetInt32(20),
                });
            }
            return outList;
        }
    }

    public List<DownloadJobRow> ListJobs(string sessionId, int limit = 5000)
    {
        lock (_gate)
        {
            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();
            using var cmd = conn.CreateCommand();
            cmd.CommandText = @"
SELECT
  session_id, job_index,
  track_id, track_title, track_artists, track_album,
  state, path, bytes, error
FROM download_job
WHERE session_id = $sid
ORDER BY job_index ASC
LIMIT $limit;";
            cmd.Parameters.AddWithValue("$sid", sessionId ?? "");
            cmd.Parameters.AddWithValue("$limit", Math.Clamp(limit, 1, 20000));

            using var r = cmd.ExecuteReader();
            var outList = new List<DownloadJobRow>();
            while (r.Read())
            {
                outList.Add(new DownloadJobRow
                {
                    SessionId = r.GetString(0),
                    JobIndex = r.GetInt32(1),
                    TrackId = r.IsDBNull(2) ? null : r.GetString(2),
                    TrackTitle = r.IsDBNull(3) ? null : r.GetString(3),
                    TrackArtists = r.IsDBNull(4) ? null : r.GetString(4),
                    TrackAlbum = r.IsDBNull(5) ? null : r.GetString(5),
                    State = r.GetString(6),
                    Path = r.IsDBNull(7) ? null : r.GetString(7),
                    Bytes = r.IsDBNull(8) ? null : r.GetInt64(8),
                    Error = r.IsDBNull(9) ? null : r.GetString(9),
                });
            }
            return outList;
        }
    }

    public void DeleteSession(string sessionId)
    {
        lock (_gate)
        {
            using var conn = new SqliteConnection(ConnectionString);
            conn.Open();
            using var cmd = conn.CreateCommand();
            cmd.CommandText = "DELETE FROM download_session WHERE session_id = $sid;";
            cmd.Parameters.AddWithValue("$sid", sessionId ?? "");
            cmd.ExecuteNonQuery();
        }
    }

    public static string FormatUnixMs(long unixMs)
    {
        try
        {
            var dt = DateTimeOffset.FromUnixTimeMilliseconds(unixMs).ToLocalTime();
            return dt.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture);
        }
        catch
        {
            return unixMs.ToString(CultureInfo.InvariantCulture);
        }
    }
}


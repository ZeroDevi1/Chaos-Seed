using System;
using System.IO;
using ChaosSeed.WinUI3.Services.Downloads;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class MusicDownloadDbTests
{
    [Fact]
    public void InsertAndQuery_SessionAndJobs()
    {
        var dir = Path.Combine(Path.GetTempPath(), "chaosseed-tests");
        Directory.CreateDirectory(dir);

        var path = Path.Combine(dir, $"musicdl-{Guid.NewGuid():N}.sqlite3");
        try
        {
            var db = new MusicDownloadDb(path);

            var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            db.UpsertSession(new DownloadSessionRow
            {
                SessionId = "sid1",
                StartedAtUnixMs = now,
                LastUpdateUnixMs = now,
                Done = false,
                TargetType = "track",
                Service = "qq",
                Title = "t",
                Artist = "a",
                Album = "alb",
                CoverUrl = null,
                OutDir = "C:\\out",
                QualityId = "flac",
                PathTemplate = null,
                Overwrite = false,
                Concurrency = 3,
                Retries = 2,
                Total = 1,
                DoneCount = 0,
                Failed = 0,
                Skipped = 0,
                Canceled = 0,
            });

            db.UpsertJobs("sid1", new[]
            {
                new DownloadJobRow
                {
                    SessionId = "sid1",
                    JobIndex = 0,
                    TrackId = "tid",
                    TrackTitle = "t",
                    TrackArtists = "a",
                    TrackAlbum = "alb",
                    State = "pending",
                    Path = null,
                    Bytes = null,
                    Error = null,
                }
            });

            var sessions = db.ListSessions(limit: 10);
            Assert.Single(sessions);
            Assert.Equal("sid1", sessions[0].SessionId);

            var jobs = db.ListJobs("sid1", limit: 10);
            Assert.Single(jobs);
            Assert.Equal(0, jobs[0].JobIndex);
            Assert.Equal("pending", jobs[0].State);

            db.UpsertJobs("sid1", new[]
            {
                new DownloadJobRow
                {
                    SessionId = "sid1",
                    JobIndex = 0,
                    TrackId = "tid",
                    TrackTitle = null,
                    TrackArtists = null,
                    TrackAlbum = null,
                    State = "done",
                    Path = "C:\\out\\x.flac",
                    Bytes = 1234,
                    Error = null,
                }
            });

            jobs = db.ListJobs("sid1", limit: 10);
            Assert.Single(jobs);
            Assert.Equal("done", jobs[0].State);
            Assert.Equal("t", jobs[0].TrackTitle);
            Assert.Equal(1234, jobs[0].Bytes);
        }
        finally
        {
            TryDelete(path);
            TryDelete(path + "-wal");
            TryDelete(path + "-shm");
        }
    }

    [Fact]
    public void DeleteSession_CascadesJobs()
    {
        var dir = Path.Combine(Path.GetTempPath(), "chaosseed-tests");
        Directory.CreateDirectory(dir);

        var path = Path.Combine(dir, $"musicdl-{Guid.NewGuid():N}.sqlite3");
        try
        {
            var db = new MusicDownloadDb(path);
            var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            db.UpsertSession(new DownloadSessionRow
            {
                SessionId = "sid2",
                StartedAtUnixMs = now,
                LastUpdateUnixMs = now,
                Done = true,
                TargetType = "album",
                Service = "qq",
                Title = "alb",
                Artist = "a",
                Album = "alb",
                CoverUrl = null,
                OutDir = "C:\\out",
                QualityId = "flac",
                PathTemplate = null,
                Overwrite = false,
                Concurrency = 3,
                Retries = 2,
                Total = 1,
                DoneCount = 1,
                Failed = 0,
                Skipped = 0,
                Canceled = 0,
            });
            db.UpsertJobs("sid2", new[]
            {
                new DownloadJobRow
                {
                    SessionId = "sid2",
                    JobIndex = 0,
                    TrackId = "t",
                    TrackTitle = "t",
                    TrackArtists = "a",
                    TrackAlbum = "alb",
                    State = "done",
                }
            });

            Assert.Single(db.ListJobs("sid2", limit: 10));
            db.DeleteSession("sid2");
            Assert.Empty(db.ListSessions(limit: 10));
            Assert.Empty(db.ListJobs("sid2", limit: 10));
        }
        finally
        {
            TryDelete(path);
            TryDelete(path + "-wal");
            TryDelete(path + "-shm");
        }
    }

    private static void TryDelete(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch
        {
            // ignore
        }
    }
}


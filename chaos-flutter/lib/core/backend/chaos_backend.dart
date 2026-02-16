import '../models/danmaku.dart';
import '../models/live.dart';
import '../models/live_directory.dart';
import '../models/lyrics.dart';
import '../models/music.dart';
import '../models/now_playing.dart';
import '../models/subtitles.dart';

abstract class ChaosBackend {
  String get name;

  // Live directory.
  Future<List<LiveDirCategory>> categories(String site);
  Future<LiveDirRoomListResult> recommendRooms(String site, int page);
  Future<LiveDirRoomListResult> categoryRooms(
    String site,
    String? parentId,
    String categoryId,
    int page,
  );
  Future<LiveDirRoomListResult> searchRooms(
      String site, String keyword, int page);

  // Live playback.
  Future<LivestreamDecodeManifestResult> decodeManifest(String input);
  Future<LiveOpenResult> openLive(String input, {String? variantId});
  Future<void> closeLive(String sessionId);

  Stream<DanmakuMessage> danmakuStream(String sessionId);
  Future<DanmakuFetchImageResult> fetchDanmakuImage(
      String sessionId, String url);

  // Lyrics / now playing.
  Future<NowPlayingSnapshot> nowPlayingSnapshot(
      NowPlayingSnapshotParams params);
  Future<List<LyricsSearchResult>> lyricsSearch(LyricsSearchParams params);

  // Music.
  Future<void> musicConfigSet(MusicProviderConfig cfg);
  Future<List<MusicTrack>> searchTracks(MusicSearchParams p);
  Future<List<MusicAlbum>> searchAlbums(MusicSearchParams p);
  Future<List<MusicArtist>> searchArtists(MusicSearchParams p);
  Future<List<MusicTrack>> albumTracks(MusicAlbumTracksParams p);
  Future<List<MusicAlbum>> artistAlbums(MusicArtistAlbumsParams p);
  Future<MusicTrackPlayUrlResult> trackPlayUrl(MusicTrackPlayUrlParams p);
  Future<MusicLoginQr> qqLoginQrCreate(String loginType);
  Future<MusicLoginQrPollResult> qqLoginQrPoll(String sessionId);
  Future<QqMusicCookie> qqRefreshCookie(QqMusicCookie cookie);
  Future<MusicLoginQr> kugouLoginQrCreate(String loginType);
  Future<MusicLoginQrPollResult> kugouLoginQrPoll(String sessionId);
  Future<MusicDownloadStartResult> downloadStart(MusicDownloadStartParams p);
  Future<MusicDownloadStatus> downloadStatus(String sessionId);
  Future<void> downloadCancel(String sessionId);

  // Subtitles.
  Future<List<ThunderSubtitleItem>> subtitleSearch(SubtitleSearchParams p);
  Future<SubtitleDownloadReply> subtitleDownload(SubtitleDownloadParams p);

  Future<void> dispose();
}

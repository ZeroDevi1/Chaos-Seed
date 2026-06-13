// 直播间详情页：展示清晰度列表并调用 IINA 播放

const { useState, useEffect } = React;

function DetailView({ room, onBack, onShowToast }) {
  const [qualities, setQualities] = useState([]);
  const [selectedId, setSelectedId] = useState(null);
  const [loading, setLoading] = useState(false);
  const [logs, setLogs] = useState('');

  useEffect(() => {
    setLoading(true);
    setSelectedId(null);
    setLogs('正在解析直播间信息…');
    setTimeout(() => {
      const qs = window.makeQualities();
      setQualities(qs);
      setLogs(`已解析 ${qs.length} 个清晰度/线路组合。`);
      setLoading(false);
    }, 600);
  }, [room.id]);

  function handlePlay() {
    const q = qualities.find(x => x.id === selectedId);
    if (!q) return;
    setLogs(`正在启动 IINA 播放「${q.name} · ${q.line}」…`);
    setTimeout(() => {
      onShowToast(`已通过 IINA 打开「${room.title}」`);
      setLogs(`IINA 已启动：${room.title} / ${q.name} / ${q.line}`);
    }, 500);
  }

  function handleCopyUrl() {
    onShowToast('已复制直连 URL 到剪贴板（原型模拟）');
  }

  const platformConfig = window.platforms.find(p => p.id === room.platform) || window.platforms[0];

  return (
    <>
      <div className="toolbar">
        <div className="toolbar-title">直播间详情</div>
        <div className="toolbar-spacer" />
      </div>
      <div className="scroll-content">
        <div className="back-link" onClick={onBack}>
          <IconChevronLeft size={14} />
          返回首页
        </div>

        <div className="detail-header">
          <div className="detail-cover">封面占位</div>
          <div className="detail-meta">
            <div className="detail-title">{room.title}</div>
            <div className="detail-subtitle">
              <span className={`platform-badge ${platformConfig.badgeClass}`} style={{ marginRight: 8 }}>
                {platformConfig.label}
              </span>
              主播：{room.streamer} · 在线：{window.formatViewers(room.viewers)}
            </div>
            <div className="detail-actions">
              <Button
                variant="primary"
                onClick={handlePlay}
                disabled={!selectedId || loading}
              >
                <IconPlay size={14} />
                在 IINA 中播放
              </Button>
              <Button variant="secondary" onClick={handleCopyUrl}>
                <IconCopy size={14} />
                复制 URL
              </Button>
            </div>
          </div>
        </div>

        <div className="quality-section">
          <div className="quality-section-title">清晰度 / 线路</div>
          {loading ? (
            <div className="empty-state" style={{ minHeight: 160 }}>
              <div className="loading-spinner" />
            </div>
          ) : (
            qualities.map(q => (
              <div
                key={q.id}
                className={`quality-row ${selectedId === q.id ? 'selected' : ''}`}
                onClick={() => setSelectedId(q.id)}
              >
                <div>
                  <div className="quality-name">{q.name}</div>
                  <div className="quality-line">{q.line}</div>
                </div>
                <div className={`quality-status ${q.resolved ? '' : 'pending'}`}>
                  {q.resolved ? '已解析' : '需二段解析'}
                </div>
              </div>
            ))
          )}
        </div>

        <div style={{ marginTop: 16, padding: 12, borderRadius: 10, background: 'rgba(128,128,128,0.08)', fontSize: 12, color: 'var(--text-secondary)', fontFamily: 'Menlo, Monaco, monospace' }}>
          {logs}
        </div>
      </div>
    </>
  );
}

Object.assign(window, { DetailView });

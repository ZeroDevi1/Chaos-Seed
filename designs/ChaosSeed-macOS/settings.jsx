// 设置页：daemon 路径、IINA 路径、主题切换

const { useState } = React;

function SettingsView({ theme, onToggleTheme }) {
  const [daemonPath, setDaemonPath] = useState('/Applications/ChaosSeed.app/Contents/Resources/chaos-daemon');
  const [iinaPath, setIinaPath] = useState('/Applications/IINA.app');
  const [timeout, setTimeout] = useState(30);
  const [daemonRunning, setDaemonRunning] = useState(true);

  return (
    <>
      <div className="toolbar">
        <div className="toolbar-title">设置</div>
        <div className="toolbar-spacer" />
        <div className="icon-button" title="切换深色/浅色模式" onClick={onToggleTheme}>
          {theme === 'dark' ? <IconSun size={16} /> : <IconMoon size={16} />}
        </div>
      </div>
      <div className="scroll-content">
        <div className="settings-form">
          <div className="settings-row">
            <div>
              <div className="settings-label">chaos-daemon 路径</div>
              <div className="settings-desc">负责直播源解析的本地 JSON-RPC 服务。</div>
            </div>
            <input
              className="settings-input"
              value={daemonPath}
              onChange={e => setDaemonPath(e.target.value)}
            />
          </div>

          <div className="settings-row">
            <div>
              <div className="settings-label">IINA 应用路径</div>
              <div className="settings-desc">用于播放解析后的直播流。</div>
            </div>
            <input
              className="settings-input"
              value={iinaPath}
              onChange={e => setIinaPath(e.target.value)}
            />
          </div>

          <div className="settings-row">
            <div>
              <div className="settings-label">网络超时（秒）</div>
              <div className="settings-desc">调用 daemon 解析时的最大等待时间。</div>
            </div>
            <input
              className="settings-input"
              type="number"
              min={5}
              max={120}
              value={timeout}
              onChange={e => setTimeout(Number(e.target.value))}
            />
          </div>

          <div className="settings-row">
            <div>
              <div className="settings-label">Daemon 状态</div>
              <div className="settings-desc">
                {daemonRunning ? '服务运行中，可正常解析。' : '服务未运行，解析将失败。'}
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <span style={{ fontSize: 13, color: 'var(--text-secondary)', display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className={`status-dot ${daemonRunning ? '' : 'offline'}`} />
                {daemonRunning ? '运行中' : '已停止'}
              </span>
              <Button
                variant={daemonRunning ? 'secondary' : 'primary'}
                onClick={() => setDaemonRunning(r => !r)}
              >
                {daemonRunning ? '停止' : '启动'}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

Object.assign(window, { SettingsView });

// 应用入口：导航、主题、页面路由

const { useState, useEffect } = React;

function App() {
  const [screen, setScreen] = useState('home');
  const [selectedRoom, setSelectedRoom] = useState(null);
  const [theme, setTheme] = useState('light');
  const [toast, setToast] = useState(null);

  // 同步主题属性到 html
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  function showToast(message) {
    setToast(message);
    setTimeout(() => setToast(null), 2200);
  }

  function openRoom(room) {
    setSelectedRoom(room);
    setScreen('detail');
  }

  function backToHome() {
    setScreen('home');
    setSelectedRoom(null);
  }

  const navItems = [
    { id: 'home', label: '首页', icon: IconHome },
    { id: 'history', label: '历史', icon: IconHistory },
    { id: 'settings', label: '设置', icon: IconSettings },
  ];

  return (
    <div className="app-shell" data-screen-label="main-window">
      <div className="title-bar">
        <div className="traffic-lights">
          <div className="traffic-light red" />
          <div className="traffic-light yellow" />
          <div className="traffic-light green" />
        </div>
        <div className="window-title">Chaos Seed</div>
      </div>

      <div className="main-layout">
        <div className="sidebar">
          <div className="sidebar-section">导航</div>
          {navItems.map(item => {
            const Icon = item.icon;
            return (
              <div
                key={item.id}
                className={`nav-item ${screen === item.id ? 'active' : ''}`}
                onClick={() => setScreen(item.id)}
              >
                <span className="nav-icon"><Icon size={18} /></span>
                {item.label}
              </div>
            );
          })}
        </div>

        <div className="content-area">
          {screen === 'home' && <HomeView onOpenRoom={openRoom} />}
          {screen === 'history' && (
            <>
              <div className="toolbar">
                <div className="toolbar-title">历史</div>
                <div className="toolbar-spacer" />
              </div>
              <div className="scroll-content">
                <EmptyState
                  title="暂无历史记录"
                  description="播放过的直播间会出现在这里，方便快速回访。"
                  icon={<IconHistory size={40} />}
                />
              </div>
            </>
          )}
          {screen === 'detail' && selectedRoom && (
            <DetailView
              room={selectedRoom}
              onBack={backToHome}
              onShowToast={showToast}
            />
          )}
          {screen === 'settings' && (
            <SettingsView
              theme={theme}
              onToggleTheme={() => setTheme(t => t === 'light' ? 'dark' : 'light')}
            />
          )}
        </div>
      </div>

      {toast && <Toast message={toast} />}
    </div>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);

// 共享展示组件

function RoomCard({ room, onClick }) {
  const platformConfig = window.platforms.find(p => p.id === room.platform) || window.platforms[0];
  return (
    <div className="room-card" onClick={() => onClick(room)}>
      <div className="room-cover">
        封面占位
        <div className="room-viewers">
          <IconEye size={12} />
          {window.formatViewers(room.viewers)}
        </div>
      </div>
      <div className="room-info">
        <div className="room-title" title={room.title}>{room.title}</div>
        <div className="room-streamer">
          <span className={`platform-badge ${platformConfig.badgeClass}`}>{platformConfig.label}</span>
          <span>{room.streamer}</span>
        </div>
      </div>
    </div>
  );
}

function SegmentedControl({ options, value, onChange }) {
  return (
    <div className="segmented-control">
      {options.map(opt => (
        <div
          key={opt.id}
          className={`segment ${value === opt.id ? 'active' : ''}`}
          onClick={() => onChange(opt.id)}
        >
          {opt.label}
        </div>
      ))}
    </div>
  );
}

function SearchBox({ value, onChange, onSubmit, placeholder = '搜索主播或标题…' }) {
  return (
    <div className="search-box">
      <IconSearch size={15} />
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={e => onChange(e.target.value)}
        onKeyDown={e => e.key === 'Enter' && onSubmit()}
      />
    </div>
  );
}

function EmptyState({ title, description, icon }) {
  return (
    <div className="empty-state">
      {icon && <div style={{ marginBottom: 16, opacity: 0.6 }}>{icon}</div>}
      <div className="empty-state-title">{title}</div>
      <div className="empty-state-desc">{description}</div>
    </div>
  );
}

function Toast({ message }) {
  return <div className="toast">{message}</div>;
}

function Button({ children, variant = 'secondary', onClick, disabled = false, title }) {
  return (
    <button
      className={`button button-${variant}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
    >
      {children}
    </button>
  );
}

Object.assign(window, {
  RoomCard,
  SegmentedControl,
  SearchBox,
  EmptyState,
  Toast,
  Button,
});

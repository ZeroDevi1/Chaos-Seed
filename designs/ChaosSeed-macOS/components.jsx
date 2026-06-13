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

function Modal({ title, children, onClose, onConfirm, confirmText = '确认', confirmDisabled = false }) {
  return (
    <div
      style={{
        position: 'fixed', inset: 0, zIndex: 200,
        background: 'rgba(0,0,0,0.35)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        backdropFilter: 'blur(4px)',
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: 420,
          background: 'var(--surface-elevated)',
          borderRadius: 14,
          boxShadow: '0 20px 60px rgba(0,0,0,0.35)',
          border: '1px solid var(--border)',
          overflow: 'hidden',
        }}
        onClick={e => e.stopPropagation()}
      >
        <div
          style={{
            padding: '16px 20px',
            borderBottom: '1px solid var(--border)',
            fontSize: 15,
            fontWeight: 700,
            color: 'var(--text)',
          }}
        >
          {title}
        </div>
        <div style={{ padding: 20 }}>
          {children}
        </div>
        <div
          style={{
            padding: '12px 20px 16px',
            display: 'flex', justifyContent: 'flex-end', gap: 10,
          }}
        >
          <Button variant="secondary" onClick={onClose}>取消</Button>
          <Button variant="primary" onClick={onConfirm} disabled={confirmDisabled}>{confirmText}</Button>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, {
  RoomCard,
  SegmentedControl,
  SearchBox,
  EmptyState,
  Toast,
  Button,
  Modal,
});

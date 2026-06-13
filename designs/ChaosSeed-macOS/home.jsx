// 首页：平台目录 + 搜索 + 直播间卡片网格

const { useState, useEffect } = React;

function HomeView({ onOpenRoom }) {
  const [platform, setPlatform] = useState('bili_live');
  const [keyword, setKeyword] = useState('');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [page, setPage] = useState(1);
  const [rooms, setRooms] = useState([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const pageSize = 6;

  function load() {
    setLoading(true);
    setTimeout(() => {
      const res = window.makeRooms(platform, page, pageSize, searchKeyword);
      setRooms(res.rooms);
      setTotal(res.total);
      setLoading(false);
    }, 350);
  }

  useEffect(() => {
    setPage(1);
    load();
  }, [platform, searchKeyword]);

  useEffect(() => {
    load();
  }, [page]);

  function handleSearch() {
    setSearchKeyword(keyword);
    setPage(1);
  }

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <>
      <div className="toolbar">
        <div className="toolbar-title">首页</div>
        <SegmentedControl
          options={window.platforms}
          value={platform}
          onChange={setPlatform}
        />
        <div className="toolbar-spacer" />
        <SearchBox
          value={keyword}
          onChange={setKeyword}
          onSubmit={handleSearch}
          placeholder="搜索主播或标题…"
        />
        <div
          className="icon-button"
          title="刷新"
          onClick={() => load()}
        >
          <IconRefresh size={16} />
        </div>
      </div>

      <div className="scroll-content">
        {loading ? (
          <div className="empty-state">
            <div className="loading-spinner" />
          </div>
        ) : rooms.length === 0 ? (
          <EmptyState
            title={searchKeyword ? '未找到匹配结果' : '暂无直播间'}
            description={searchKeyword ? '换个关键词试试看。' : '当前分类下没有可展示的直播间。'}
            icon={<IconSearch size={40} />}
          />
        ) : (
          <>
            <div className="room-grid">
              {rooms.map(room => (
                <RoomCard key={room.id} room={room} onClick={onOpenRoom} />
              ))}
            </div>
            <div className="pagination">
              <button
                className="page-btn"
                disabled={page <= 1}
                onClick={() => setPage(p => p - 1)}
              >
                上一页
              </button>
              <div className="page-info">{page} / {totalPages}</div>
              <button
                className="page-btn"
                disabled={page >= totalPages}
                onClick={() => setPage(p => p + 1)}
              >
                下一页
              </button>
            </div>
          </>
        )}
      </div>
    </>
  );
}

Object.assign(window, { HomeView });

// 模拟数据：平台、直播间、清晰度等
const platforms = [
  { id: 'bili_live', label: 'BiliLive', badgeClass: 'bili' },
  { id: 'douyu', label: 'Douyu', badgeClass: 'douyu' },
  { id: 'huya', label: 'Huya', badgeClass: 'huya' },
];

function makeCategories(platform) {
  const categories = {
    bili_live: [
      { id: 'recommend', name: '推荐', children: [] },
      { id: 'game', name: '网游', children: [{ id: 'lol', name: '英雄联盟' }, { id: 'dota2', name: 'DOTA2' }, { id: 'cs2', name: 'CS2' }] },
      { id: 'mobile', name: '手游', children: [{ id: 'honkai', name: '星穹铁道' }, { id: 'genshin', name: '原神' }, { id: 'mahjong', name: '雀魂' }] },
      { id: 'single', name: '单机', children: [{ id: 'host', name: '主机游戏' }, { id: 'indie', name: '独立游戏' }] },
      { id: 'ent', name: '娱乐', children: [{ id: 'sing', name: '唱见' }, { id: 'radio', name: '电台' }] },
    ],
    douyu: [
      { id: 'recommend', name: '推荐', children: [] },
      { id: 'lol', name: '英雄联盟', children: [] },
      { id: 'dota2', name: 'DOTA2', children: [] },
      { id: 'cs2', name: 'CS2', children: [] },
      { id: 'mobile', name: '手游', children: [{ id: 'king', name: '王者荣耀' }, { id: 'pubgm', name: '和平精英' }] },
      { id: 'ent', name: '娱乐', children: [{ id: '户外', name: '户外' }, { id: 'music', name: '音乐' }] },
    ],
    huya: [
      { id: 'recommend', name: '推荐', children: [] },
      { id: '1', name: '网游', children: [{ id: 'lol', name: '英雄联盟' }, { id: 'dnf', name: 'DNF' }] },
      { id: '2', name: '单机', children: [{ id: 'mc', name: '我的世界' }, { id: 'pubg', name: '绝地求生' }] },
      { id: '3', name: '手游', children: [{ id: 'king', name: '王者荣耀' }, { id: 'codm', name: '使命召唤手游' }] },
      { id: '8', name: '娱乐', children: [{ id: 'sing', name: '星秀' }, { id: 'talk', name: '脱口秀' }] },
    ],
  };
  return categories[platform] || [];
}

function makeRooms(platform, page, pageSize, keyword, categoryId, subCategoryId) {
  const all = {
    bili_live: [
      { id: 'b1', title: '【星穹铁道】新版本主线开荒', streamer: '老沐', viewers: 124300, tags: ['mobile', 'honkai'] },
      { id: 'b2', title: '雀魂麻将 段位场打工', streamer: '杆菌无敌', viewers: 8720, tags: ['mobile', 'mahjong'] },
      { id: 'b3', title: '炉石传说 竞技场12胜', streamer: '异灵术老师', viewers: 156000, tags: ['game'] },
      { id: 'b4', title: '深夜歌回 / 点歌台', streamer: '泠鸢yousa', viewers: 42100, tags: ['ent', 'sing'] },
      { id: 'b5', title: 'CS2 完美S局', streamer: '玩机器Machine', viewers: 38900, tags: ['game', 'cs2'] },
      { id: 'b6', title: '原神 每日+深渊', streamer: '棉花大哥哥', viewers: 21500, tags: ['mobile', 'genshin'] },
      { id: 'b7', title: '英雄联盟 韩服王者局', streamer: 'Uzi', viewers: 456000, tags: ['game', 'lol'] },
      { id: 'b8', title: 'DOTA2 路人单排', streamer: 'Sccc', viewers: 32100, tags: ['game', 'dota2'] },
    ],
    douyu: [
      { id: 'd1', title: 'LPL 夏季赛 官方直播间', streamer: '英雄联盟赛事', viewers: 2100000, tags: ['lol'] },
      { id: 'd2', title: 'DOTA2 主播带你看比赛', streamer: 'Zard', viewers: 56200, tags: ['dota2'] },
      { id: 'd3', title: '街头霸王6 练习房', streamer: '小孩曾卓君', viewers: 28900, tags: [] },
      { id: 'd4', title: '主机游戏 新作试玩', streamer: '女流66', viewers: 134000, tags: [] },
      { id: 'd5', title: '户外直播 城市探索', streamer: '彡彡九户外', viewers: 7600, tags: ['ent', '户外'] },
      { id: 'd6', title: '王者荣耀 巅峰赛', streamer: '张大仙', viewers: 980000, tags: ['mobile', 'king'] },
    ],
    huya: [
      { id: 'h1', title: '穿越火线 CFPL 职业联赛', streamer: 'CFPL', viewers: 430000, tags: ['1'] },
      { id: 'h2', title: '绝地求生 PCL 训练赛', streamer: '4AM战队', viewers: 125000, tags: ['2', 'pubg'] },
      { id: 'h3', title: '云顶之弈 新版本上分', streamer: '神超', viewers: 67000, tags: ['1', 'lol'] },
      { id: 'h4', title: '永劫无间 三排冲榜', streamer: '法神', viewers: 43200, tags: ['1'] },
      { id: 'h5', title: 'FIFA Online 4 排位', streamer: 'A胖', viewers: 18900, tags: ['1'] },
      { id: 'h6', title: '魔兽争霸3 怀旧对战', streamer: 'infi', viewers: 54300, tags: ['2', 'mc'] },
    ],
  };
  let list = all[platform] || [];
  if (keyword) {
    const k = keyword.toLowerCase();
    list = list.filter(r => r.title.toLowerCase().includes(k) || r.streamer.toLowerCase().includes(k));
  } else if (categoryId && categoryId !== 'recommend') {
    list = list.filter(r => r.tags.includes(categoryId) || r.tags.includes(subCategoryId));
  }
  const total = list.length;
  const start = (page - 1) * pageSize;
  const paged = list.slice(start, start + pageSize);
  return {
    rooms: paged.map(r => ({ ...r, platform })),
    total,
    hasMore: start + pageSize < total,
  };
}

function makeQualities() {
  return [
    { id: '原画', name: '原画', line: '线路1', resolved: true },
    { id: '蓝光', name: '蓝光', line: '线路1', resolved: true },
    { id: '超清', name: '超清', line: '线路2', resolved: true },
    { id: '高清', name: '高清', line: '线路1', resolved: false },
    { id: '流畅', name: '流畅', line: '线路3', resolved: true },
  ];
}

function formatViewers(n) {
  if (n >= 10000) return (n / 10000).toFixed(1) + '万';
  return n.toLocaleString();
}

Object.assign(window, {
  platforms,
  makeCategories,
  makeRooms,
  makeQualities,
  formatViewers,
});

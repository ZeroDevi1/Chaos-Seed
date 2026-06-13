// 模拟数据：平台、直播间、清晰度等
const platforms = [
  { id: 'bili_live', label: 'BiliLive', badgeClass: 'bili' },
  { id: 'douyu', label: 'Douyu', badgeClass: 'douyu' },
  { id: 'huya', label: 'Huya', badgeClass: 'huya' },
];

function makeRooms(platform, page, pageSize, keyword) {
  const all = {
    bili_live: [
      { id: 'b1', title: '【星穹铁道】新版本主线开荒', streamer: '老沐', viewers: 124300 },
      { id: 'b2', title: '雀魂麻将 段位场打工', streamer: '杆菌无敌', viewers: 8720 },
      { id: 'b3', title: '炉石传说 竞技场12胜', streamer: '异灵术老师', viewers: 156000 },
      { id: 'b4', title: '深夜歌回 / 点歌台', streamer: '泠鸢yousa', viewers: 42100 },
      { id: 'b5', title: 'CS2 完美S局', streamer: '玩机器Machine', viewers: 38900 },
      { id: 'b6', title: '原神 每日+深渊', streamer: '棉花大哥哥', viewers: 21500 },
    ],
    douyu: [
      { id: 'd1', title: 'LPL 夏季赛 官方直播间', streamer: '英雄联盟赛事', viewers: 2100000 },
      { id: 'd2', title: 'DOTA2 主播带你看比赛', streamer: 'Zard', viewers: 56200 },
      { id: 'd3', title: '街头霸王6 练习房', streamer: '小孩曾卓君', viewers: 28900 },
      { id: 'd4', title: '主机游戏 新作试玩', streamer: '女流66', viewers: 134000 },
      { id: 'd5', title: '户外直播 城市探索', streamer: '彡彡九户外', viewers: 7600 },
      { id: 'd6', title: '王者荣耀 巅峰赛', streamer: '张大仙', viewers: 980000 },
    ],
    huya: [
      { id: 'h1', title: '穿越火线 CFPL 职业联赛', streamer: 'CFPL', viewers: 430000 },
      { id: 'h2', title: '绝地求生 PCL 训练赛', streamer: '4AM战队', viewers: 125000 },
      { id: 'h3', title: '云顶之弈 新版本上分', streamer: '神超', viewers: 67000 },
      { id: 'h4', title: '永劫无间 三排冲榜', streamer: '法神', viewers: 43200 },
      { id: 'h5', title: 'FIFA Online 4 排位', streamer: 'A胖', viewers: 18900 },
      { id: 'h6', title: '魔兽争霸3 怀旧对战', streamer: 'infi', viewers: 54300 },
    ],
  };
  let list = all[platform] || [];
  if (keyword) {
    const k = keyword.toLowerCase();
    list = list.filter(r => r.title.toLowerCase().includes(k) || r.streamer.toLowerCase().includes(k));
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
  makeRooms,
  makeQualities,
  formatViewers,
});

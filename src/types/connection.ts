// 网络连接类型定义
export interface TcpConnection {
  id: string; // 稳定唯一标识（协议+五元组），用于行 key、选中比较与差分
  protocol: string;
  local_addr: string;
  local_port: number;
  remote_addr: string;
  remote_port: number;
  state: string;
  pid: number | null;
  process_name: string | null;
  exe_path: string | null; // 可执行文件路径，用于从快照的 icons 映射中取图标
  icon: string | null; // Base64 encoded icon data（由前端从快照合并而来）
  start_time: number | null; // Process start time in seconds since Unix epoch
  isNew?: boolean; // 标记是否为新连接
  hasChanged?: boolean; // 标记连接状态是否有变化
  isDeleted?: boolean; // 标记是否为即将删除的连接
}

// get_connections 返回快照：图标按 exe_path 去重，避免每行重复传输
export interface NetRate {
  down_bps: number; // 系统下载速率（字节/秒）
  up_bps: number; // 系统上传速率（字节/秒）
  total_down: number; // 会话累计下载（字节，应用启动以来）
  total_up: number; // 会话累计上传（字节，应用启动以来）
}

// 应用偏好设置（托盘可切换的开关项）
export interface AppSettings {
  close_to_quit: boolean;
  show_net: boolean; // 状态栏是否显示网络流量
}

export interface ConnectionsSnapshot {
  connections: TcpConnection[];
  icons: Record<string, string>;
  net_rate: NetRate;
}

// 进程详情类型定义
export interface ProcessDetails {
  pid: number;
  name: string;
  command_line: string;
  executable_path: string;
  memory_usage: number;
  cpu_usage: number;
  parent_pid: number | null; // 修复：与后端 Option<u32> 保持一致
  start_time: number;
}

// 调用链节点：祖先链与子进程列表共用
export interface LineageNode {
  pid: number;
  ppid: number | null;
  name: string;
  command_line: string;
  exe_path: string | null;
  start_time: number | null;
  user: string | null;
  icon: string | null; // Base64 PNG，后端按 exe_path 复用全局图标缓存
  parent_exited: boolean; // 该节点的父进程已退出（PID 被回收复用或已消失），链在此断裂
  is_target: boolean; // 是否为当前查看的目标进程
}

// 进程调用链：沿 PPID 向上追溯到 PID 1 的完整祖先链 + 直接子进程
export interface ProcessLineage {
  nodes: LineageNode[]; // root → target 顺序
  children: LineageNode[]; // 目标进程的直接子进程
}

// 端口绑定条目：get_port_bindings 按端口实时解析的结果
export interface PortBinding {
  protocol: string; // TCP / UDP
  local_addr: string;
  state: string; // LISTEN / ESTABLISHED / UNCONN ...
  pid: number | null;
  process_name: string | null;
}

// 排序相关类型
export type SortColumn =
  | 'process_name'
  | 'pid'
  | 'protocol'
  | 'local_addr'
  | 'local_port'
  | 'remote_addr'
  | 'remote_port'
  | 'state'
  | 'start_time';

export type SortDirection = 'asc' | 'desc';

// 排序值类型（用于类型安全）
export type SortValue = string | number | null;

// 过滤器类型
export interface FilterState {
  protocol: 'all' | 'TCP' | 'UDP';
  state: string;
  searchProcessName: string;
  searchLocalPort: string;
}

// 状态栏信息类型
export interface StatusBarInfo {
  totalConnections: number;
  tcpCount: number;
  udpCount: number;
  lastRefresh: string;
}

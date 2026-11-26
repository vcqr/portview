<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "vue-i18n";
import ConnectionsTable from "./components/ConnectionsTable.vue";
import MenuBar from "./components/MenuBar.vue";
import StatusBar from "./components/StatusBar.vue";
import ContextMenu from "./components/ContextMenu.vue";
import ProcessDrawer from "./components/ProcessDrawer.vue";
import AboutDialog from "./components/AboutDialog.vue";
import MessageBox from "./components/MessageBox.vue";
import type { TcpConnection, ProcessDetails, ProcessLineage, PortBinding, SortColumn, SortDirection, ConnectionsSnapshot, NetRate, AppSettings } from "@/types/connection";

// 初始化国际化
const { t, locale } = useI18n();

const rawConnections = ref<TcpConnection[]>([]); // 后端原始快照（未筛选）
const lastView = ref<TcpConnection[]>([]); // 上一轮筛选后的视图，供差分比较
// 平局行的随机次序（按连接身份持久）：快照本身是 TCP 块+UDP 块，
// 稳定排序会保留这种分组；随机序号让平局行协议无关且跨刷新稳定
const sortSeeds = new Map<string, number>();
const isLoading = ref(false);
// 自动刷新相关状态
const autoRefreshInterval = ref<number | null>(null);
const isAutoRefreshEnabled = ref(false);
const refreshIntervals = [1, 2, 3, 5, 10]; // 可选的刷新间隔（秒）
const isFirstLoad = ref(true); // 标记是否是首次加载

const selectedRefreshInterval = ref(1); // 默认选择1秒

const DELETED_CONNECTION_TTL = 5000; // 已删除连接在列表中保留展示的时长（毫秒）
// 已删除连接的暂存区：id -> { conn, deletedAt }，展示期内持续显示
const deletedConnections = ref(
  new Map<string, { conn: TcpConnection; deletedAt: number }>(),
);
let consecutiveLoadFailures = 0; // 连续加载失败次数，用于避免自动刷新期间反复弹窗

// 连接的稳定唯一标识
function makeConnId(conn: TcpConnection): string {
  return `${conn.protocol}-${conn.local_addr}-${conn.local_port}-${conn.remote_addr}-${conn.remote_port}-${conn.pid || "null"}`;
}

// 用于跟踪筛选条件是否发生变化
const previousFilterProtocol = ref("all");
const previousFilterState = ref("all");
const previousSearchProcessName = ref("");
const previousSearchLocalPort = ref("");

// 主题相关状态
const isDarkMode = ref(false);

// 右键菜单相关状态
const showContextMenu = ref(false);
const contextMenuPosition = ref({ x: 0, y: 0 });
const selectedConnection = ref<TcpConnection | null>(null);

// 选中行相关状态
const clickedConnection = ref<TcpConnection | null>(null);

// 进程详情抽屉相关状态
const showProcessDetails = ref(false);
const processDetails = ref<ProcessDetails | null>(null);
const selectedPid = ref<number | null>(null); // 当前详情抽屉展示的进程
const processGone = ref(false); // 轮询时发现进程已退出
const processRefreshing = ref(false); // 手动刷新进行中
let processDetailsTimer: number | null = null;
// 进程调用链（打开抽屉时拉取一次；断链标记 parent_exited 由后端判定）
const processLineage = ref<ProcessLineage | null>(null);
const lineageLoading = ref(false);
// 抽屉内的进程导航历史：双击链上/子进程节点进入新进程时压栈，
// 返回按钮逐级弹出；从连接表格发起的打开视为新的浏览起点（清空历史）
const lineageNavHistory = ref<number[]>([]);
const LINEAGE_NAV_HISTORY_MAX = 20;
// 从调用链点击节点后在连接列表中高亮的 PID（null = 未启用）
const lineageHighlightPid = ref<number | null>(null);
// 详情抽屉宽度（左缘可拖拽调整，持久化到 localStorage）
const drawerWidth = ref<number>(
  Number.parseFloat(localStorage.getItem("drawerWidth") || "") || 400,
);
const connectionsTableRef = ref<InstanceType<typeof ConnectionsTable> | null>(
  null,
);

// 获取进程详情；silent 用于轮询场景——进程退出时不弹错误框。
// 返回是否成功
async function loadProcessDetails(pid: number, silent = false): Promise<boolean> {
  try {
    const details: ProcessDetails = await invoke("get_process_details", {
      pid,
    });
    processDetails.value = details;
    processGone.value = false;
    return true;
  } catch (error) {
    if (silent) {
      // 进程可能已退出：停止轮询，弹窗内提示
      processGone.value = true;
      stopProcessDetailsRefresh();
      return false;
    }
    console.error(t("alerts.getProcessDetailsFailed", { error }), error);
    showMessageDialogFn(t("alerts.getProcessDetailsFailed", { error }), 'error');
    return false;
  }
}

// 弹窗打开期间每秒刷新当前进程信息
function startProcessDetailsRefresh(pid: number) {
  stopProcessDetailsRefresh();
  processDetailsTimer = window.setInterval(() => {
    loadProcessDetails(pid, true);
  }, 1000);
}

function stopProcessDetailsRefresh() {
  if (processDetailsTimer !== null) {
    clearInterval(processDetailsTimer);
    processDetailsTimer = null;
  }
}

// 关闭抽屉时停止刷新并清除数据，避免下次打开闪现上一个进程的旧信息；
// 同时清除调用链高亮与导航历史
function setProcessDetailsVisible(visible: boolean) {
  showProcessDetails.value = visible;
  if (!visible) {
    stopProcessDetailsRefresh();
    processDetails.value = null;
    processLineage.value = null;
    lineageHighlightPid.value = null;
    lineageNavHistory.value = [];
  }
}

// 手动刷新当前进程信息
async function refreshProcessDetails() {
  if (selectedPid.value === null || processRefreshing.value) return;
  processRefreshing.value = true;
  await loadProcessDetails(selectedPid.value, true);
  processRefreshing.value = false;
}

// 拉取当前进程的调用链（打开抽屉或手动重新追踪时调用；
// 调用链基本静态，不随秒级轮询刷新）
async function loadProcessLineage(pid: number) {
  lineageLoading.value = true;
  try {
    processLineage.value = await invoke<ProcessLineage>(
      "get_process_lineage",
      { pid },
    );
  } catch (error) {
    console.error(t("alerts.getLineageFailed", { error }), error);
    showNotification(t("alerts.getLineageFailed", { error }), "error");
  } finally {
    lineageLoading.value = false;
  }
}

// 手动重新追踪调用链
function refreshProcessLineage() {
  if (selectedPid.value === null || lineageLoading.value) return;
  loadProcessLineage(selectedPid.value);
}

// 调用链/子进程节点单击：在连接列表中高亮该 PID 的行并滚动定位；
// 再次点击同一节点取消高亮。链上祖先进程大多没有网络连接，提示用户
function highlightPidInTable(pid: number) {
  if (lineageHighlightPid.value === pid) {
    lineageHighlightPid.value = null;
    return;
  }
  lineageHighlightPid.value = pid;
  void nextTick(() => {
    const found = connectionsTableRef.value?.scrollToPid(pid) ?? false;
    if (!found) {
      showNotification(t("alerts.noConnectionsForPid"), "info");
    }
  });
}

// 加载指定进程到抽屉（不处理导航历史；由 inspectProcess / goBack 调用）
async function loadProcessView(pid: number) {
  selectedPid.value = pid;
  lineageHighlightPid.value = null;
  processGone.value = false;
  processDetails.value = null;
  processLineage.value = null;
  showProcessDetails.value = true;
  const ok = await loadProcessDetails(pid);
  if (!ok) {
    setProcessDetailsVisible(false);
    return;
  }
  startProcessDetailsRefresh(pid);
  loadProcessLineage(pid);
}

// 双击调用链/子进程节点：把当前进程压入导航历史后切换为查看该进程
async function inspectProcess(pid: number) {
  if (!showProcessDetails.value) {
    lineageNavHistory.value = [];
    await loadProcessView(pid);
    return;
  }
  if (selectedPid.value === pid) return;
  if (selectedPid.value !== null) {
    lineageNavHistory.value.push(selectedPid.value);
    if (lineageNavHistory.value.length > LINEAGE_NAV_HISTORY_MAX) {
      lineageNavHistory.value.shift();
    }
  }
  await loadProcessView(pid);
}

// 返回上一进程：弹出导航历史栈顶并恢复查看
function goBackInLineage() {
  const prev = lineageNavHistory.value.pop();
  if (prev === undefined) return;
  loadProcessView(prev);
}

// 抽屉拖宽：更新并持久化
function onDrawerWidthChange(width: number) {
  drawerWidth.value = width;
  localStorage.setItem("drawerWidth", String(width));
}

// 菜单栏"追踪"入口：输入端口号或 PID 直接打开调用链，不依赖表格行。
// 数字输入优先按端口实时解析（端口是本应用主域），无绑定再回退按 PID
const traceInProgress = ref(false);
async function traceTarget(input: string) {
  if (traceInProgress.value) return;
  const trimmed = input.trim();
  if (!/^\d+$/.test(trimmed)) {
    showNotification(t("alerts.traceInvalidInput"), "error");
    return;
  }
  const num = Number(trimmed);
  traceInProgress.value = true;
  try {
    // ① 按端口解析：多个进程共享端口时优先选 LISTEN 的那个
    let bindings: PortBinding[] = [];
    try {
      bindings = await invoke<PortBinding[]>("get_port_bindings", { port: num });
    } catch {
      // 端口枚举失败视为无绑定，继续按 PID 尝试
    }
    const withPid = bindings.filter((b) => b.pid);
    const target =
      withPid.find((b) => b.state === "LISTEN") ?? withPid[0];
    if (target?.pid) {
      const pidCount = new Set(withPid.map((b) => b.pid)).size;
      if (pidCount > 1) {
        showNotification(
          t("alerts.tracePortShared", {
            port: num,
            count: pidCount,
            pid: target.pid,
          }),
          "info",
        );
      }
      lineageNavHistory.value = [];
      selectedConnection.value = null;
      await loadProcessView(target.pid);
      return;
    }

    // ② 端口无绑定：按 PID 尝试（对不存在的进程后端会报错）
    try {
      await invoke("get_process_details", { pid: num });
    } catch {
      showNotification(t("alerts.traceNotFound", { input: trimmed }), "error");
      return;
    }
    lineageNavHistory.value = [];
    selectedConnection.value = null;
    await loadProcessView(num);
  } finally {
    traceInProgress.value = false;
  }
}

// 显示进程详情抽屉：先清除上一个进程的数据，打开后再拉取；
// 拉取失败时关闭空抽屉（错误信息已单独提示）。
// 从连接表格发起属于新的浏览起点，清空导航历史
async function showProcessDetailsDialog(conn: TcpConnection) {
  if (!conn.pid) return;
  selectedConnection.value = conn; // 保存选中的连接以获取图标信息
  lineageNavHistory.value = [];
  await loadProcessView(conn.pid);
}

// 关于对话框相关状态
const showAbout = ref(false);

// 应用版本号
const appVersion = ref('Loading...');

// 通知提示相关状态
const showNotificationBox = ref(false);
const notificationMessage = ref('');
const notificationType = ref<'success' | 'error' | 'info'>('info'); // 'success', 'error', or 'info'

// 消息弹窗相关状态
const showMessageDialog = ref(false);
const messageDialogTitle = ref('');
const messageDialogContent = ref('');
const messageDialogType = ref<'info' | 'success' | 'warning' | 'error'>('info');

// 排序相关状态
const sortColumn = ref<SortColumn | null>(null);
const sortDirection = ref<SortDirection>("asc"); // 'asc' 升序, 'desc' 降序

// 存储用户自定义的列宽
const customColumnWidths = ref<Record<string, number>>({});

// 菜单栏筛选条件
const filterProtocol = ref<"all" | "TCP" | "UDP">("all");
const filterState = ref("all");
const searchProcessName = ref("");
const searchLocalPort = ref("");

// 状态栏信息
const statusBarInfo = ref({
  totalConnections: 0,
  tcpConnections: 0,
  udpConnections: 0,
  kernelConnections: 0,
  establishedConnections: 0,
  listenConnections: 0,
  timeWaitConnections: 0,
  closeWaitConnections: 0,
  otherConnections: 0,
  netDown: 0, // 系统下载速率（字节/秒）
  netUp: 0, // 系统上传速率（字节/秒）
  netTotalDown: 0, // 会话累计下载（字节）
  netTotalUp: 0, // 会话累计上传（字节）
  lastUpdate: new Date().toLocaleTimeString(),
  refreshInterval: null as number | null,
});

// 系统网速实时刷新（独立 1 秒轻量轮询，不依赖自动刷新开关）
let netRateTimer: number | null = null;

async function refreshNetRate() {
  try {
    const rate = await invoke<NetRate>("get_net_rate");
    statusBarInfo.value.netDown = rate.down_bps;
    statusBarInfo.value.netUp = rate.up_bps;
    statusBarInfo.value.netTotalDown = rate.total_down;
    statusBarInfo.value.netTotalUp = rate.total_up;
  } catch {
    // 浏览器环境或后端不可用时静默忽略
  }
}

// 状态栏网络流量展示开关（托盘"显示网络流量"控制）：
// 关闭时同步停掉每秒的网速轮询，开启时立即恢复并拉取一次
const showNetTraffic = ref(true);
let settingsUnlisten: (() => void) | null = null;

function setShowNetTraffic(enabled: boolean) {
  showNetTraffic.value = enabled;
  if (enabled) {
    refreshNetRate();
    if (netRateTimer === null) {
      netRateTimer = window.setInterval(refreshNetRate, 1000);
    }
  } else if (netRateTimer !== null) {
    window.clearInterval(netRateTimer);
    netRateTimer = null;
  }
}

// 头部筛选条件应用到列表（与展示列表同一套逻辑）
function filterConnections(list: TcpConnection[]): TcpConnection[] {
  let out = list;
  if (filterProtocol.value !== "all") {
    out = out.filter((conn) => conn.protocol === filterProtocol.value);
  }
  if (filterState.value !== "all") {
    out = out.filter((conn) => conn.state === filterState.value);
  }
  const nameTerm = searchProcessName.value.toLowerCase().trim();
  if (nameTerm !== "") {
    out = out.filter(
      (conn) =>
        conn.process_name &&
        conn.process_name.toLowerCase().includes(nameTerm),
    );
  }
  const portTerm = searchLocalPort.value.trim();
  if (portTerm !== "") {
    out = out.filter((conn) =>
      conn.local_port.toString().startsWith(portTerm),
    );
  }
  return out;
}

// 排序比较器：只按点击的列决定次序，其他列不参与；
// 平局行由稳定排序保持快照中的自然顺序
function compareConns(a: TcpConnection, b: TcpConnection): number {
  let result = 0;
  switch (sortColumn.value) {
    case "pid":
      result = (a.pid || 0) - (b.pid || 0);
      break;
    case "local_port":
      result = a.local_port - b.local_port;
      break;
    case "remote_port":
      result = a.remote_port - b.remote_port;
      break;
    case "start_time":
      result = (a.start_time || 0) - (b.start_time || 0);
      break;
    case "process_name":
      result = (a.process_name || "").localeCompare(b.process_name || "");
      break;
    case "protocol":
      result = a.protocol.localeCompare(b.protocol);
      break;
    case "local_addr":
      result = a.local_addr.localeCompare(b.local_addr);
      break;
    case "remote_addr":
      result = a.remote_addr.localeCompare(b.remote_addr);
      break;
    case "state":
      result = a.state.localeCompare(b.state);
      break;
  }
  if (result === 0) {
    // 平局按持久的随机序号排列：打破快照自带的 TCP/UDP 分组，
    // 且不引入其他列、跨刷新稳定
    result = (sortSeeds.get(a.id) ?? 0) - (sortSeeds.get(b.id) ?? 0);
  }
  return sortDirection.value === "asc" ? result : -result;
}

// 展示列表 = 筛选后的原始数据 + 展示期内的已删除连接，再应用排序。
// 筛选/排序全部由头部状态派生，点击即时生效，无需重新拉取数据。
const connections = computed<TcpConnection[]>(() => {
  const filtered = filterConnections(rawConnections.value);
  const currentIds = new Set(filtered.map((conn) => conn.id));
  // 已删除连接同样应用当前筛选，避免切换筛选后残留其它协议/状态的行
  const deletedRows = filterConnections(
    [...deletedConnections.value.values()].map((entry) => entry.conn),
  ).filter((conn) => !currentIds.has(conn.id));
  const list = [...filtered, ...deletedRows];
  return sortColumn.value ? [...list].sort(compareConns) : list;
});

// 获取网络连接列表
async function loadConnections() {
  // 上一轮请求未完成时跳过本次触发，避免请求堆积
  if (isLoading.value) return;
  isLoading.value = true;
  try {
    const result: ConnectionsSnapshot = await invoke("get_connections");

    // 检查筛选条件是否发生了变化
    const filterChanged =
      previousFilterProtocol.value !== filterProtocol.value ||
      previousFilterState.value !== filterState.value ||
      previousSearchProcessName.value !== searchProcessName.value ||
      previousSearchLocalPort.value !== searchLocalPort.value;

    // 更新筛选条件记录
    previousFilterProtocol.value = filterProtocol.value;
    previousFilterState.value = filterState.value;
    previousSearchProcessName.value = searchProcessName.value;
    previousSearchLocalPort.value = searchLocalPort.value;

    // 生成稳定 id：相同五元组+PID 的连接真实存在（如 Chrome 的多个 mDNS
    // 套接字，SO_REUSEADDR），追加出现序号保证 id 唯一——重复 id 会让
    // Vue 的列表 diff 失效，表现为筛选后旧行残留、行序错乱
    const list = result.connections;
    const idCount = new Map<string, number>();
    for (const conn of list) {
      const base = makeConnId(conn);
      const n = (idCount.get(base) ?? 0) + 1;
      idCount.set(base, n);
      conn.id = n === 1 ? base : `${base}#${n}`;
      if (!sortSeeds.has(conn.id)) {
        sortSeeds.set(conn.id, Math.random());
      }
      conn.icon = (conn.exe_path && result.icons[conn.exe_path]) || null;
    }
    // 清理已消失连接的随机序号（id 已生成，直接用）
    const seenIds = new Set(list.map((conn) => conn.id));
    for (const id of [...sortSeeds.keys()]) {
      if (!seenIds.has(id)) {
        sortSeeds.delete(id);
      }
    }

    // 当前筛选视图（与展示列表同一套筛选逻辑），供差分比较
    const filteredResult = filterConnections(list);
    const currentIds = new Set(filteredResult.map((conn) => conn.id));

    // 如果筛选条件发生了变化，则不使用上一次的视图做比较
    const previousFilteredConnections: TcpConnection[] =
      !filterChanged && !isFirstLoad.value ? lastView.value : [];
    const previousById = new Map(
      previousFilteredConnections.map((conn) => [conn.id, conn]),
    );

    // 标记状态变化与新增连接（仅在非首次加载且筛选条件未变化时）
    if (!isFirstLoad.value && !filterChanged) {
      filteredResult.forEach((conn) => {
        const prevConn = previousById.get(conn.id);
        if (prevConn) {
          // 与上一轮的同 id 连接比较，状态变化才标记
          conn.hasChanged = prevConn.state !== conn.state;
          conn.isNew = false;
        } else {
          // 没有匹配的旧连接，是新连接
          conn.isNew = true;
          conn.hasChanged = false;
        }
      });

      // 本轮消失的连接进入暂存区，持续展示 5 秒
      const now = Date.now();
      previousFilteredConnections.forEach((prevConn) => {
        if (!currentIds.has(prevConn.id)) {
          deletedConnections.value.set(prevConn.id, {
            conn: {
              ...prevConn,
              isDeleted: true,
              isNew: false,
              hasChanged: false,
            },
            deletedAt: now,
          });
        }
      });

      if (deletedConnections.value.size > 0) {
        // 兜底定时器：自动刷新关闭时也能按时移除过期行
        window.setTimeout(
          purgeExpiredDeletedConnections,
          DELETED_CONNECTION_TTL + 100,
        );
      }
    } else {
      // 首次加载或筛选条件发生变化时，不标记任何连接为新增或删除
      filteredResult.forEach((conn) => {
        conn.isNew = false;
        conn.hasChanged = false;
        conn.isDeleted = false;
      });
      deletedConnections.value.clear();
    }

    // 清理暂存区：超时的、以及重新出现的连接
    for (const [id, entry] of deletedConnections.value) {
      if (
        currentIds.has(id) ||
        Date.now() - entry.deletedAt > DELETED_CONNECTION_TTL
      ) {
        deletedConnections.value.delete(id);
      }
    }

    // 检查是否有连接状态发生了变化（仅用于日志）
    const changedConnections = filteredResult.filter((conn) => conn.hasChanged);
    if (changedConnections.length > 0) {
      console.log(
        `${t("alerts.connectionStateChanged", { count: changedConnections.length })}:`,
        changedConnections.map((c) => ({
          protocol: c.protocol,
          local_addr: c.local_addr,
          local_port: c.local_port,
          remote_addr: c.remote_addr,
          remote_port: c.remote_port,
          old_state: previousById.get(c.id)?.state,
          new_state: c.state,
        })),
      );
    }

    // 更新状态栏信息（使用原始结果，不受筛选影响）
    updateStatusBarInfo(result.connections);

    // 标记不再是首次加载
    isFirstLoad.value = false;
    consecutiveLoadFailures = 0;

    // 记录本轮筛选视图供下一轮差分；原始快照更新后，
    // 展示列表（筛选+排序）由 computed 自动派生
    lastView.value = filteredResult;
    rawConnections.value = list;
  } catch (error) {
    consecutiveLoadFailures += 1;
    console.error(t("alerts.getConnectionsFailed", { error }), error);
    // 仅在首次失败时弹窗，避免自动刷新期间每秒弹出一次
    if (consecutiveLoadFailures === 1) {
      showMessageDialogFn(t("alerts.getConnectionsFailed", { error }), 'error');
    }
  } finally {
    isLoading.value = false;
  }
}

// 筛选变更时向后台拉取一次最新数据（本地由 computed 即时生效，
// 后台拉取负责数据实时反馈；防重入保证与自动刷新轮询不堆积）
function refetchForFilterChange() {
  loadConnections();
}

// 文本搜索输入防抖拉取（300ms），避免每个按键都触发一次全量枚举
let searchDebounceTimer: number | null = null;
function scheduleSearchRefetch() {
  if (searchDebounceTimer !== null) {
    window.clearTimeout(searchDebounceTimer);
  }
  searchDebounceTimer = window.setTimeout(refetchForFilterChange, 300);
}

function onFilterStateChange(state: string) {
  filterState.value = state;
  refetchForFilterChange();
}

function onSearchProcessNameInput(value: string) {
  searchProcessName.value = value;
  scheduleSearchRefetch();
}

function onSearchLocalPortInput(value: string) {
  searchLocalPort.value = value;
  scheduleSearchRefetch();
}

// 设置协议筛选（列表由 computed 派生，改动即时生效）
function setProtocolFilter(protocol: "all" | "TCP" | "UDP") {
  filterProtocol.value = protocol;
  refetchForFilterChange();
}

// 更新状态栏信息
function updateStatusBarInfo(connections: TcpConnection[]) {
  statusBarInfo.value.totalConnections = connections.length;
  statusBarInfo.value.tcpConnections = connections.filter(
    (conn) => conn.protocol === "TCP",
  ).length;
  statusBarInfo.value.udpConnections = connections.filter(
    (conn) => conn.protocol === "UDP",
  ).length;
  statusBarInfo.value.kernelConnections = connections.filter(
    (conn) =>
      conn.process_name === "[KERNEL]" ||
      (conn.process_name && conn.process_name.includes("[KERNEL]")),
  ).length;

  // 统计各种连接状态的数量
  statusBarInfo.value.establishedConnections = connections.filter(
    (conn) => conn.state === "ESTABLISHED",
  ).length;
  statusBarInfo.value.listenConnections = connections.filter(
    (conn) => conn.state === "LISTEN",
  ).length;
  statusBarInfo.value.timeWaitConnections = connections.filter(
    (conn) => conn.state === "TIME_WAIT",
  ).length;
  statusBarInfo.value.closeWaitConnections = connections.filter(
    (conn) => conn.state === "CLOSE_WAIT",
  ).length;
  statusBarInfo.value.otherConnections = connections.filter(
    (conn) =>
      conn.state !== "ESTABLISHED" &&
      conn.state !== "LISTEN" &&
      conn.state !== "TIME_WAIT" &&
      conn.state !== "CLOSE_WAIT",
  ).length;

  statusBarInfo.value.lastUpdate = new Date().toLocaleTimeString();
}

// 杀死进程
async function killProcess(conn: TcpConnection) {
  if (conn.pid) {
    try {
      await invoke("kill_process", { pid: conn.pid });
      // 成功杀死进程后，重新加载连接列表
      await loadConnections();
      showNotification(
        t("alerts.processKilled", {
          name: conn.process_name || "Unknown",
          pid: conn.pid,
        }),
        'success'
      );
    } catch (error) {
      console.error(t("alerts.processKillFailed", { error }), error);
      showNotification(
        t("alerts.processKillFailed", { error }),
        'error'
      );
    }
  }
}

// 打开进程所在目录
async function openContainingFolder(conn: TcpConnection) {
  if (conn.pid) {
    try {
      // 获取进程详细信息以获得可执行路径
      const details: ProcessDetails = await invoke("get_process_details", {
        pid: conn.pid,
      });
      
      // 检查可执行路径是否存在且不为空
      if (details.executable_path && details.executable_path.trim() !== '') {
        // 调用打开目录功能
        await invoke('open_folder', { path: details.executable_path });
      } else {
        // 如果可执行路径为空，显示提示信息
        showNotification(
          t("alerts.noExecutablePath", { name: conn.process_name || "Unknown" }),
          'info'
        );
      }
    } catch (error) {
      console.error(t("alerts.openFolderFailed", { error }), error);
      showNotification(
        t("alerts.openFolderFailed", { error }),
        'error'
      );
    }
  }
}

// 显示右键菜单
function showContextMenuHandler(conn: TcpConnection, event: MouseEvent) {
  event.preventDefault(); // 阻止默认右键菜单

  selectedConnection.value = conn;
  showContextMenu.value = true;

  // 计算菜单位置，确保不会超出屏幕边界
  const menuWidth = 150; // 菜单宽度
  const menuHeight = 60; // 菜单高度

  let posX = event.clientX;
  let posY = event.clientY;

  // 检查是否超出右侧屏幕边界
  if (posX + menuWidth > window.innerWidth) {
    posX = window.innerWidth - menuWidth - 5; // 5px 边距
  }

  // 检查是否超出底部屏幕边界
  if (posY + menuHeight > window.innerHeight) {
    posY = window.innerHeight - menuHeight - 5; // 5px 边距
  }

  contextMenuPosition.value = {
    x: posX,
    y: posY,
  };
}

// 隐藏右键菜单
function hideContextMenu() {
  showContextMenu.value = false;
}

// 移除暂存区中超过展示时长的已删除连接（展示列表由 computed 派生）
function purgeExpiredDeletedConnections() {
  const now = Date.now();
  for (const [id, entry] of deletedConnections.value) {
    if (now - entry.deletedAt > DELETED_CONNECTION_TTL) {
      deletedConnections.value.delete(id);
    }
  }
}

// 切换列排序（排序由 computed 派生，无需重新拉取数据）
function toggleSort(column: SortColumn) {
  if (sortColumn.value === column) {
    // 如果当前列已经是排序列，则切换排序方向
    sortDirection.value = sortDirection.value === "asc" ? "desc" : "asc";
  } else {
    // 如果切换到新列，则默认升序
    sortColumn.value = column;
    sortDirection.value = "asc";
  }
}

// 启用自动刷新
function enableAutoRefresh() {
  if (autoRefreshInterval.value !== null) {
    clearInterval(autoRefreshInterval.value);
  }

  isAutoRefreshEnabled.value = true;
  autoRefreshInterval.value = window.setInterval(() => {
    loadConnections();
  }, selectedRefreshInterval.value * 1000); // 转换为毫秒

  // 更新状态栏信息
  statusBarInfo.value.refreshInterval = selectedRefreshInterval.value;
}

// 禁用自动刷新
function disableAutoRefresh() {
  if (autoRefreshInterval.value !== null) {
    clearInterval(autoRefreshInterval.value);
    autoRefreshInterval.value = null;
  }

  isAutoRefreshEnabled.value = false;
  statusBarInfo.value.refreshInterval = null;
}

// 切换自动刷新
function toggleAutoRefresh() {
  if (isAutoRefreshEnabled.value) {
    disableAutoRefresh();
  } else {
    enableAutoRefresh();
  }
}

// 更改刷新间隔
function changeRefreshInterval(interval: number) {
  selectedRefreshInterval.value = interval;

  // 如果自动刷新已启用，重新设置间隔
  if (isAutoRefreshEnabled.value) {
    enableAutoRefresh();
  }

  // 更新状态栏信息
  if (isAutoRefreshEnabled.value) {
    statusBarInfo.value.refreshInterval = selectedRefreshInterval.value;
  }
}

// 页面加载完成后自动获取连接列表
onMounted(async () => {
  // 设置语言为本地存储的语言或浏览器语言
  const savedLang = localStorage.getItem("locale");
  if (savedLang && (savedLang === "zh" || savedLang === "en")) {
    const { locale } = useI18n();
    locale.value = savedLang as "zh" | "en";
  }

  // 检查并应用主题偏好
  checkSystemThemePreference();

  // 主题已应用后再显示窗口，避免启动白屏（tauri.conf 中窗口初始为隐藏）
  try {
    await getCurrentWindow().show();
  } catch {
    // 浏览器环境没有 Tauri 窗口，忽略
  }

  loadConnections();

  // 系统网速实时刷新（受托盘"显示网络流量"开关控制）
  setShowNetTraffic(true);

  // 同步托盘偏好，并监听托盘开关变更
  try {
    const settings = await invoke<AppSettings>("get_app_settings");
    setShowNetTraffic(settings.show_net);
  } catch {
    // 浏览器环境忽略
  }
  try {
    settingsUnlisten = await listen<boolean>("show-net-changed", (event) => {
      setShowNetTraffic(event.payload);
    });
  } catch {
    // 浏览器环境忽略
  }

  // 获取应用版本
  try {
    appVersion.value = await invoke("get_app_version");
  } catch (error) {
    console.error('Failed to get app version:', error);
    appVersion.value = 'Unknown';
  }

  // 监听文档上的点击事件，用于隐藏右键菜单
  document.addEventListener("click", hideContextMenu);

  // 禁用原生右键菜单
  document.addEventListener("contextmenu", disableNativeContextMenu);

  // 监视主题变化
  watch(
    isDarkMode,
    (newVal) => {
      if (newVal) {
        document.documentElement.classList.add("dark");
        localStorage.setItem("theme", "dark");
      } else {
        document.documentElement.classList.remove("dark");
        localStorage.setItem("theme", "light");
      }
    },
    { immediate: true },
  );
});

// 组件卸载时清理定时器和事件监听器
onUnmounted(() => {
  // 清除自动刷新定时器
  if (autoRefreshInterval.value !== null) {
    clearInterval(autoRefreshInterval.value);
  }

  // 清除网速刷新定时器
  if (netRateTimer !== null) {
    clearInterval(netRateTimer);
  }

  // 取消托盘偏好事件监听
  if (settingsUnlisten !== null) {
    settingsUnlisten();
    settingsUnlisten = null;
  }

  // 停止进程详情刷新
  stopProcessDetailsRefresh();

  // 移除文档点击事件监听器
  document.removeEventListener("click", hideContextMenu);
  
  // 移除右键菜单事件监听器
  document.removeEventListener("contextmenu", disableNativeContextMenu);
});

// 禁用原生右键菜单的函数
function disableNativeContextMenu(e: MouseEvent) {
  // 总是阻止原生右键菜单，让应用的自定义右键菜单处理所有情况
  e.preventDefault();
}

// 语言切换功能
function changeLanguage(lang: "zh" | "en") {
  locale.value = lang;
  localStorage.setItem("locale", lang); // 保存用户选择的语言到localStorage
}

// 主题切换功能
function toggleTheme() {
  isDarkMode.value = !isDarkMode.value;
  if (isDarkMode.value) {
    document.documentElement.classList.add("dark");
    localStorage.setItem("theme", "dark");
  } else {
    document.documentElement.classList.remove("dark");
    localStorage.setItem("theme", "light");
  }
}

// 显示通知
function showNotification(message: string, type: 'success' | 'error' | 'info' = 'info') {
  notificationMessage.value = message;
  notificationType.value = type;
  showNotificationBox.value = true;

  // 3秒后自动隐藏通知
  setTimeout(() => {
    showNotificationBox.value = false;
  }, 3000);
}

// 显示消息弹窗
function showMessageDialogFn(content: string, type: 'info' | 'success' | 'warning' | 'error' = 'error', title: string = t('alerts.dialogTitle')) {
  messageDialogContent.value = content;
  messageDialogType.value = type;
  messageDialogTitle.value = title;
  showMessageDialog.value = true;
}

// 消息弹窗确认回调
function onMessageDialogConfirm() {
  showMessageDialog.value = false;
}

// 检查系统主题偏好
function checkSystemThemePreference() {
  const savedTheme = localStorage.getItem("theme");
  const systemPrefersDark = window.matchMedia(
    "(prefers-color-scheme: dark)",
  ).matches;

  if (savedTheme) {
    isDarkMode.value = savedTheme === "dark";
  } else {
    isDarkMode.value = systemPrefersDark;
  }

  // 应用主题到页面
  if (isDarkMode.value) {
    document.documentElement.classList.add("dark");
  } else {
    document.documentElement.classList.remove("dark");
  }
}
</script>

<template>
  <div class="container">
    <!-- 菜单栏 -->
    <MenuBar
      :filterProtocol="filterProtocol"
      :filterState="filterState"
      :searchProcessName="searchProcessName"
      :searchLocalPort="searchLocalPort"
      :isAutoRefreshEnabled="isAutoRefreshEnabled"
      :selectedRefreshInterval="selectedRefreshInterval"
      :refreshIntervals="refreshIntervals"
      :isDarkMode="isDarkMode"
      @update:filterProtocol="filterProtocol = $event"
      @update:filterState="onFilterStateChange"
      @update:searchProcessName="onSearchProcessNameInput($event)"
      @update:searchLocalPort="onSearchLocalPortInput($event)"
      @toggleAutoRefresh="toggleAutoRefresh"
      @changeRefreshInterval="changeRefreshInterval"
      @update:selectedRefreshInterval="selectedRefreshInterval = $event"
      @changeLanguage="changeLanguage"
      @toggleTheme="toggleTheme"
      @setProtocolFilter="setProtocolFilter"
      @showAboutDialog="showAbout = true"
      @traceTarget="traceTarget"
    />

    <!-- 中部：连接表格 + 进程详情抽屉 -->
    <div class="main-row">
      <ConnectionsTable
        ref="connectionsTableRef"
        :connections="connections"
        :clickedConnection="clickedConnection"
        :sortColumn="sortColumn"
        :sortDirection="sortDirection"
        :customColumnWidths="customColumnWidths"
        :isLoading="isLoading"
        :highlightPid="lineageHighlightPid"
        @update:clickedConnection="clickedConnection = $event"
        @toggleSort="toggleSort"
        @showContextMenuHandler="showContextMenuHandler"
        @showProcessDetailsDialog="showProcessDetailsDialog"
        @update:customColumnWidths="customColumnWidths = $event"
      />

      <!-- 进程详情抽屉（含调用链） -->
      <ProcessDrawer
        :show="showProcessDetails"
        :processDetails="processDetails"
        :lineage="processLineage"
        :lineageLoading="lineageLoading"
        :processIcon="selectedConnection?.icon || null"
        :processGone="processGone"
        :refreshing="processRefreshing"
        :highlightPid="lineageHighlightPid"
        :width="drawerWidth"
        :canGoBack="lineageNavHistory.length > 0"
        @update:show="setProcessDetailsVisible"
        @refresh="refreshProcessDetails"
        @refreshLineage="refreshProcessLineage"
        @back="goBackInLineage"
        @highlight="highlightPidInTable"
        @inspect="inspectProcess"
        @update:width="onDrawerWidthChange"
      />
    </div>

    <!-- 右键菜单 -->
    <ContextMenu
      :showContextMenu="showContextMenu"
      :contextMenuPosition="contextMenuPosition"
      :selectedConnection="selectedConnection"
      @update:showContextMenu="showContextMenu = $event"
      @showProcessDetailsDialog="showProcessDetailsDialog"
      @killProcess="killProcess"
      @openContainingFolder="openContainingFolder"
    />

    <!-- 状态栏 -->
    <StatusBar :statusBarInfo="statusBarInfo" :showNetTraffic="showNetTraffic" />

    <!-- 关于对话框 -->
    <AboutDialog
      :showAbout="showAbout"
      :appVersion="appVersion"
      @update:showAbout="showAbout = $event"
    />

    <!-- 通知提示框 -->
    <div
      v-if="showNotificationBox"
      class="notification-box"
      :class="notificationType"
    >
      <div class="notification-content">
        <span class="notification-message">{{ notificationMessage }}</span>
        <button
          class="notification-close-btn"
          @click="showNotificationBox = false"
        >
          ×
        </button>
      </div>
    </div>
    
    <!-- 消息弹窗 -->
    <MessageBox
      :show="showMessageDialog"
      :type="messageDialogType"
      :title="messageDialogTitle"
      :message="messageDialogContent"
      @confirm="onMessageDialogConfirm"
      @close="showMessageDialog = false"
    />
  </div>
</template>

<style scoped>
/* App.vue只需要保留容器的基本样式 */
.container {
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  height: 100vh; /* 使用确切的高度 */
  width: 100%; /* 使用百分比宽度，避免滚动条问题 */
  flex: 1 1 auto;
  min-height: 0; /* 允许flex子项收缩 */
  min-width: 0; /* 允许flex子项收缩 */
}

/* 中部横排：连接表格（弹性占满）；进程详情抽屉为浮层，
   以本容器为定位基准悬停在表格上方，不挤压表格布局 */
.main-row {
  position: relative;
  display: flex;
  flex: 1 1 auto;
  min-height: 0; /* 允许flex子项收缩 */
  min-width: 0;
}
</style>

<style>
.container {
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  height: 100vh; /* 使用确切的高度 */
  width: 100%; /* 使用百分比宽度，避免滚动条问题 */
  flex: 1 1 auto;
  min-height: 0; /* 允许flex子项收缩 */
  min-width: 0; /* 允许flex子项收缩 */
}

/* ===== 通知提示框 ===== */
.notification-box {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 10000;
  min-width: 300px;
  max-width: 500px;
  border-radius: var(--radius-md);
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-left: 3px solid var(--chart-cyan);
  box-shadow: var(--shadow-lg);
  animation: slideInRight 0.25s ease-out;
  overflow: hidden;
}

.notification-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
}

.notification-message {
  flex: 1;
  margin-right: 10px;
  word-break: break-word;
  font-size: 13px;
  color: var(--text-1);
}

.notification-close-btn {
  background: none;
  border: none;
  font-size: 16px;
  cursor: pointer;
  color: var(--text-3);
  padding: 0;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  font-weight: bold;
  box-sizing: border-box;
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: normal;
  transition: background-color 0.15s ease, color 0.15s ease;
}

.notification-close-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-1);
}

/* 语义色左条 */
.notification-box.success {
  border-left-color: var(--success);
}

.notification-box.error {
  border-left-color: var(--danger);
}

.notification-box.info {
  border-left-color: var(--chart-cyan);
}

@keyframes slideInRight {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}
</style>

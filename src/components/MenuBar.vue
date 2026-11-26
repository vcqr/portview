<template>
  <!-- 主菜单容器：按功能分组，空间不足时自动换行 -->
  <div class="menu-bar">
    <div class="menu-cluster">
      <div class="menu-group">
        <span class="menu-label">{{ t("menu.protocol") }}</span>
        <div class="segmented">
          <button
            :class="['segment-btn', { active: filterProtocol === 'all' }]"
            @click="setProtocolFilter('all')"
          >
            {{ t("menu.protocolAll") }}
          </button>
          <button
            :class="['segment-btn', { active: filterProtocol === 'TCP' }]"
            @click="setProtocolFilter('TCP')"
          >
            {{ t("menu.protocolTCP") }}
          </button>
          <button
            :class="['segment-btn', { active: filterProtocol === 'UDP' }]"
            @click="setProtocolFilter('UDP')"
          >
            {{ t("menu.protocolUDP") }}
          </button>
        </div>
      </div>

      <div class="menu-group">
        <span class="menu-label">{{ t("menu.state") }}</span>
        <DropdownSelect
          class="state-select"
          :modelValue="filterState"
          :options="stateOptions"
          @update:modelValue="handleStateChange"
        />
      </div>
    </div>

    <div class="menu-cluster">
      <div class="menu-group">
        <span class="menu-label">{{ t("menu.searchProcess") }}</span>
        <input
          type="text"
          :value="searchProcessName"
          @input="handleSearchProcessInput"
          :placeholder="t('menu.searchPlaceholder')"
          class="control-input"
        />
      </div>

      <div class="menu-group">
        <span class="menu-label">{{ t("menu.searchLocalPort") }}</span>
        <input
          type="text"
          :value="searchLocalPort"
          @input="handleSearchLocalPortInput"
          :placeholder="t('menu.localPortPlaceholder')"
          class="control-input control-input--port"
        />
      </div>
    </div>

    <div class="menu-cluster">
      <div class="menu-group">
        <span class="menu-label">{{ t("menu.trace") }}</span>
        <input
          type="text"
          v-model="traceInput"
          :placeholder="t('menu.tracePlaceholder')"
          class="control-input trace-input"
          @keydown.enter="traceTarget"
        />
        <button class="control-btn" @click="traceTarget">
          <Waypoints :size="13" class="btn-icon" />
          {{ t("menu.traceButton") }}
        </button>
      </div>
    </div>

    <div class="menu-cluster">
      <div class="menu-group">
        <span class="menu-label">{{ t("menu.autoRefresh") }}</span>
        <button
          :class="['control-btn', 'refresh-toggle-btn', { active: isAutoRefreshEnabled }]"
          @click="toggleAutoRefresh"
        >
          <RefreshCw
            class="btn-icon"
            :class="{ spinning: isAutoRefreshEnabled }"
            :size="13"
          />
          {{ isAutoRefreshEnabled ? t("menu.refreshStop") : t("menu.refreshStart") }}
        </button>
        <DropdownSelect
          class="refresh-interval-select"
          :modelValue="String(selectedRefreshInterval)"
          :options="intervalOptions"
          :disabled="!isAutoRefreshEnabled"
          @update:modelValue="handleRefreshIntervalChange"
        />
      </div>
    </div>

    <div class="menu-cluster">
      <div class="menu-group">
        <span class="menu-label">{{ t("menu.language") }}</span>
        <DropdownSelect
          class="lang-select"
          :modelValue="locale"
          :options="langOptions"
          @update:modelValue="handleLanguageChange"
        />
      </div>

      <div class="menu-group">
        <button
          class="icon-btn"
          @click="handleThemeToggle"
          :title="isDarkMode ? t('menu.lightTheme') : t('menu.darkTheme')"
        >
          <Sun v-if="isDarkMode" :size="15" />
          <Moon v-else :size="15" />
        </button>
        <button class="control-btn about-btn" @click="showAboutDialog">
          {{ t("menu.about") }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, Sun, Moon, Waypoints } from "@lucide/vue";
import DropdownSelect from "./DropdownSelect.vue";

// 定义组件属性
interface Props {
  filterProtocol: "all" | "TCP" | "UDP";
  filterState: string;
  searchProcessName: string;
  searchLocalPort: string;
  isAutoRefreshEnabled: boolean;
  selectedRefreshInterval: number;
  refreshIntervals: number[];
  isDarkMode: boolean;
}

const props = defineProps<Props>();

// 定义事件发射器
interface Emits {
  (e: "update:filterProtocol", value: "all" | "TCP" | "UDP"): void;
  (e: "update:filterState", value: string): void;
  (e: "update:searchProcessName", value: string): void;
  (e: "update:searchLocalPort", value: string): void;
  (e: "toggleAutoRefresh"): void;
  (e: "changeRefreshInterval", interval: number): void;
  (e: "update:selectedRefreshInterval", value: number): void;
  (e: "changeLanguage", lang: "zh" | "en"): void;
  (e: "toggleTheme"): void;
  (e: "setProtocolFilter", protocol: "all" | "TCP" | "UDP"): void;
  (e: "showAboutDialog"): void;
  (e: "traceTarget", value: string): void;
}

const emit = defineEmits<Emits>();

// 使用国际化
const { t, locale } = useI18n();

// ===== 自定义下拉的候选项（computed 保证语言切换后文案联动） =====
const stateOptions = computed(() => [
  { value: "all", label: t("menu.stateAll") },
  { value: "LISTEN", label: t("menu.stateListen") },
  { value: "ESTABLISHED", label: t("menu.stateEstablished") },
  { value: "TIME_WAIT", label: t("menu.stateTimeWait") },
  { value: "CLOSE_WAIT", label: t("menu.stateCloseWait") },
  { value: "SYN_SENT", label: t("menu.stateSynSent") },
  { value: "SYN_RECV", label: t("menu.stateSynRecv") },
  { value: "FIN_WAIT1", label: t("menu.stateFinWait1") },
  { value: "FIN_WAIT2", label: t("menu.stateFinWait2") },
  { value: "LAST_ACK", label: t("menu.stateLastAck") },
  { value: "CLOSING", label: t("menu.stateClosing") },
  { value: "UNCONN", label: t("menu.stateUnconn") },
]);

// 刷新间隔候选项：DropdownSelect 的值统一为字符串，选中时再转回数字
const intervalOptions = computed(() =>
  props.refreshIntervals.map((n) => ({
    value: String(n),
    label: `${n}${t("menu.refreshInterval")}`,
  })),
);

const langOptions = computed(() => [
  { value: "zh", label: t("zh") },
  { value: "en", label: t("en") },
]);

// 追踪输入框：输入值仅在提交时使用，不需要同步给父组件，本地持有即可
const traceInput = ref("");

// 提交追踪（Enter 或按钮）：清空输入便于下次输入；结果由父组件通知
const traceTarget = () => {
  const value = traceInput.value.trim();
  if (value === "") return;
  emit("traceTarget", value);
  traceInput.value = "";
};

// 方法
const setProtocolFilter = (protocol: "all" | "TCP" | "UDP") => {
  emit('setProtocolFilter', protocol);
};

const toggleAutoRefresh = () => {
  emit('toggleAutoRefresh');
};

const showAboutDialog = () => {
  emit('showAboutDialog');
};

const handleStateChange = (value: string) => {
  emit('update:filterState', value);
};

const handleSearchProcessInput = (event: Event) => {
  const target = event.target as HTMLInputElement;
  emit('update:searchProcessName', target.value);
};

const handleSearchLocalPortInput = (event: Event) => {
  const target = event.target as HTMLInputElement;
  emit('update:searchLocalPort', target.value);
};

const handleRefreshIntervalChange = (value: string) => {
  const interval = Number(value);
  emit('update:selectedRefreshInterval', interval);
  emit('changeRefreshInterval', interval);
};

const handleLanguageChange = (value: string) => {
  emit('changeLanguage', value as 'zh' | 'en');
};

const handleThemeToggle = () => {
  emit('toggleTheme');
};
</script>

<style scoped>
/* 主菜单容器：按功能分组，空间不足时自动换行到下一行，
   保证所有控件始终可见（下拉面板已 Teleport 到 body，不受换行布局影响） */
.menu-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  row-gap: 6px;
  width: 100%;
  padding: 7px 8px;
  background-color: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  position: sticky;
  top: 0;
  z-index: 100;
}

/* 功能分组，竖线分隔 */
.menu-cluster {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px;
  flex-shrink: 0;
  white-space: nowrap;
}

.menu-cluster + .menu-cluster {
  border-left: 1px solid var(--border);
}

/* 第一组贴近窗口左缘，减少整体左缩进 */
.menu-cluster:first-child {
  padding-left: 4px;
}

.menu-group {
  display: flex;
  align-items: center;
  gap: 5px;
  white-space: nowrap;
  flex-shrink: 0;
}

.menu-label {
  font-size: 12px;
  color: var(--text-3);
  white-space: nowrap;
}

/* ===== 通用控件 ===== */
.control-input,
.control-btn {
  height: var(--control-height);
  padding: 0 8px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background-color: var(--bg-panel);
  color: var(--text-1);
  font-size: 12px;
  font-family: inherit;
  transition: border-color 0.15s ease, background-color 0.15s ease,
    box-shadow 0.15s ease;
}

.control-input {
  width: 108px;
  color: var(--text-1);
}

.control-input::placeholder {
  color: var(--text-3);
}

.control-input--port {
  width: 90px;
}

/* 追踪输入框：紧凑宽度，容纳端口号与 PID */
.trace-input {
  width: 84px;
  font-family: var(--font-mono);
}

.control-input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-weak);
}

.control-input:hover,
.control-btn:hover {
  border-color: var(--accent);
}

.control-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  cursor: pointer;
  font-weight: 500;
  white-space: nowrap;
}

.control-btn:active {
  transform: translateY(0.5px);
}

.btn-icon {
  flex-shrink: 0;
}

.btn-icon.spinning {
  animation: pv-spin 1.2s linear infinite;
}

/* ===== 协议分段按钮 ===== */
.segmented {
  display: flex;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  overflow: hidden;
  height: var(--control-height);
}

.segment-btn {
  padding: 0 9px;
  border: none;
  background-color: var(--bg-panel);
  color: var(--text-2);
  font-size: 12px;
  font-family: inherit;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}

.segment-btn + .segment-btn {
  border-left: 1px solid var(--border);
}

.segment-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-1);
}

.segment-btn.active {
  background-color: var(--accent);
  color: #ffffff;
  font-weight: 600;
}

/* ===== 自动刷新激活态 ===== */
.refresh-toggle-btn.active {
  background-color: var(--success-weak);
  border-color: var(--success);
  color: var(--success);
  font-weight: 600;
}

/* ===== 图标按钮（主题切换） ===== */
.icon-btn {
  height: var(--control-height);
  width: var(--control-height);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background-color: var(--bg-panel);
  color: var(--text-2);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease,
    background-color 0.15s ease;
}

.icon-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
  background-color: var(--accent-weak);
}

/* ===== 下拉宽度微调（作用于 DropdownSelect 根元素） ===== */
.state-select {
  min-width: 96px;
}

.lang-select {
  min-width: 84px;
}

.refresh-interval-select {
  min-width: 68px;
}
</style>

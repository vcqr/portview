<template>
  <div ref="rootRef" class="dd-select" :class="{ 'dd-disabled': disabled }">
    <button
      ref="triggerRef"
      type="button"
      class="dd-trigger"
      :disabled="disabled"
      :aria-expanded="open"
      @click.stop="toggle"
      @keydown="onTriggerKeydown"
    >
      <span class="dd-label">{{ displayLabel }}</span>
      <ChevronDown :size="13" class="dd-chevron" :class="{ rotated: open }" />
    </button>

    <!-- 面板 Teleport 到 body 用 fixed 定位：菜单栏带 overflow-x 滚动时，
         absolute 面板会被父级裁剪不可见；fixed 挂在 body 上不受任何裁剪 -->
    <Teleport to="body">
      <div
        v-if="open"
        ref="panelRef"
        class="dd-panel"
        role="listbox"
        :style="{
          top: panelPos.top + 'px',
          left: panelPos.left + 'px',
          minWidth: panelPos.minWidth + 'px',
        }"
      >
        <button
          v-for="(option, index) in options"
          :key="option.value"
          type="button"
          role="option"
          :aria-selected="option.value === modelValue"
          :class="[
            'dd-option',
            {
              'dd-option--selected': option.value === modelValue,
              'dd-option--active': index === activeIndex,
            },
          ]"
          @click.stop="select(option.value)"
          @mousemove="activeIndex = index"
        >
          <span class="dd-option-label">{{ option.label }}</span>
          <Check v-if="option.value === modelValue" :size="13" class="dd-check" />
        </button>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from "vue";
import { Check, ChevronDown } from "@lucide/vue";

export interface DropdownOption {
  value: string;
  label: string;
}

interface Props {
  options: DropdownOption[];
  modelValue: string;
  disabled?: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
}>();

const rootRef = ref<HTMLElement | null>(null);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const open = ref(false);
// 键盘导航高亮的候选项下标
const activeIndex = ref(0);
// 面板 fixed 定位坐标（对齐触发按钮，右缘防溢出）
const panelPos = ref({ top: 0, left: 0, minWidth: 0 });

const displayLabel = computed(
  () => props.options.find((o) => o.value === props.modelValue)?.label ?? props.modelValue,
);

async function toggle() {
  if (props.disabled) return;
  if (open.value) {
    close();
    return;
  }
  const rect = triggerRef.value?.getBoundingClientRect();
  if (!rect) return;
  panelPos.value = {
    top: rect.bottom + 4,
    left: rect.left,
    minWidth: rect.width,
  };
  open.value = true;
  if (open.value) {
    // 打开时高亮定位到当前选中项
    const idx = props.options.findIndex((o) => o.value === props.modelValue);
    activeIndex.value = idx >= 0 ? idx : 0;
    // 渲染后量宽，右缘超出视口时左移（语言下拉贴近窗口右缘的场景）
    await nextTick();
    const el = panelRef.value;
    if (el) {
      const maxLeft = window.innerWidth - el.offsetWidth - 8;
      if (panelPos.value.left > maxLeft) {
        panelPos.value = {
          ...panelPos.value,
          left: Math.max(8, maxLeft),
        };
      }
    }
  }
}

function select(value: string) {
  close();
  if (value !== props.modelValue) {
    emit("update:modelValue", value);
  }
}

function close() {
  open.value = false;
}

// 点击组件外部（触发按钮或面板之外）时关闭
function onDocumentClick(e: MouseEvent) {
  if (!open.value) return;
  const target = e.target as Node;
  if (rootRef.value?.contains(target) || panelRef.value?.contains(target)) {
    return;
  }
  close();
}

// 任意容器滚动（含菜单栏自身的横向滚动）或窗口缩放时关闭，
// 避免 fixed 面板与触发按钮错位；面板内部滚动除外
function onScrollOrResize(e: Event) {
  if (!open.value) return;
  const target = e.target as Node;
  if (rootRef.value?.contains(target) || panelRef.value?.contains(target)) {
    return;
  }
  close();
}

function onDocumentKeydown(e: KeyboardEvent) {
  if (!open.value) return;
  if (e.key === "Escape") {
    e.stopPropagation();
    close();
    return;
  }
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const delta = e.key === "ArrowDown" ? 1 : -1;
    const count = props.options.length;
    if (count === 0) return;
    activeIndex.value = (activeIndex.value + delta + count) % count;
    // 高亮项滚入可视区域
    const panel = panelRef.value;
    const item = panel?.querySelectorAll(".dd-option")[activeIndex.value];
    item?.scrollIntoView({ block: "nearest" });
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const option = props.options[activeIndex.value];
    if (option) select(option.value);
  }
}

// 触发按钮的键盘入口：Enter/Space 由原生 click 打开，ArrowDown 直接展开
function onTriggerKeydown(e: KeyboardEvent) {
  if (props.disabled) return;
  if (e.key === "ArrowDown" && !open.value) {
    e.preventDefault();
    toggle();
  }
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener("click", onDocumentClick);
    document.addEventListener("keydown", onDocumentKeydown, true);
    document.addEventListener("scroll", onScrollOrResize, true);
    window.addEventListener("resize", onScrollOrResize);
  } else {
    document.removeEventListener("click", onDocumentClick);
    document.removeEventListener("keydown", onDocumentKeydown, true);
    document.removeEventListener("scroll", onScrollOrResize, true);
    window.removeEventListener("resize", onScrollOrResize);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKeydown, true);
  document.removeEventListener("scroll", onScrollOrResize, true);
  window.removeEventListener("resize", onScrollOrResize);
});
</script>

<style scoped>
.dd-select {
  position: relative;
  display: inline-flex;
  vertical-align: middle;
}

.dd-select.dd-disabled {
  opacity: 0.55;
}

/* 触发按钮：对齐菜单栏其他控件（.control-btn）的观感 */
.dd-trigger {
  width: 100%;
  height: var(--control-height);
  padding: 0 8px;
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background-color: var(--bg-panel);
  color: var(--text-1);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 0.15s ease, background-color 0.15s ease,
    box-shadow 0.15s ease;
}

.dd-trigger:hover:not(:disabled) {
  border-color: var(--accent);
}

.dd-trigger:focus-visible {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-weak);
}

.dd-trigger:disabled {
  background-color: var(--bg-subtle);
  color: var(--text-3);
  cursor: not-allowed;
}

.dd-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dd-chevron {
  flex-shrink: 0;
  color: var(--text-3);
  transition: transform 0.15s ease;
}

.dd-chevron.rotated {
  transform: rotate(180deg);
}

/* 下拉面板：fixed 浮层，坐标由脚本按触发按钮位置计算 */
.dd-panel {
  position: fixed;
  z-index: 9990; /* 低于右键菜单(10000)，高于菜单栏(100)与抽屉(50) */
  max-height: 280px;
  overflow-y: auto;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-lg);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.dd-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 5px 8px;
  border: none;
  border-radius: calc(var(--radius-sm) - 2px);
  background: transparent;
  color: var(--text-2);
  font-size: 12px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  white-space: nowrap;
}

.dd-option:hover,
.dd-option--active {
  background: var(--bg-hover);
  color: var(--text-1);
}

.dd-option--selected {
  color: var(--accent);
  font-weight: 600;
}

.dd-option-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dd-check {
  flex-shrink: 0;
}
</style>

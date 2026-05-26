<script setup>
import { computed } from "vue";
import { systemMessage } from "../utils/message";

const items = computed(() => systemMessage.state.items);

function close(id) {
  systemMessage.close(id);
}
</script>

<template>
  <teleport to="body">
    <transition-group name="message-stack" tag="div" class="message-host">
      <div
        v-for="item in items"
        :key="item.id"
        class="message-item"
        :class="`is-${item.type}`"
      >
        <div class="message-icon">
          <svg
            v-if="item.type === 'success'"
            viewBox="0 0 24 24"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="M20 6 9 17l-5-5"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          <svg
            v-else-if="item.type === 'error'"
            viewBox="0 0 24 24"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="M12 8v5"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
            />
            <circle cx="12" cy="16.5" r="1" fill="currentColor" />
            <path
              d="M10.3 4.3h3.4L21 11.6v3.4l-7.3 7.3h-3.4L3 15v-3.4l7.3-7.3Z"
              stroke="currentColor"
              stroke-width="2"
              stroke-linejoin="round"
            />
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle
              cx="12"
              cy="12"
              r="9"
              stroke="currentColor"
              stroke-width="2"
            />
            <path
              d="M12 7.5v5"
              stroke="currentColor"
              stroke-width="2.2"
              stroke-linecap="round"
            />
            <circle cx="12" cy="16.4" r="1" fill="currentColor" />
          </svg>
        </div>

        <div class="message-body">{{ item.message }}</div>

        <button
          type="button"
          class="message-close"
          aria-label="关闭提示"
          @click="close(item.id)"
        >
          <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path
              d="M6 6l12 12M18 6 6 18"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            />
          </svg>
        </button>
      </div>
    </transition-group>
  </teleport>
</template>

<style scoped>
.message-host {
  position: fixed;
  top: 20px;
  left: 50%;
  z-index: 9999;
  width: min(420px, calc(100vw - 40px));
  display: flex;
  flex-direction: column;
  gap: 10px;
  transform: translateX(-50%);
  pointer-events: none;
}

.message-item {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 13px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  background: rgba(7, 18, 42, 0.94);
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
}

.message-item.is-success {
  border-color: rgba(34, 197, 94, 0.35);
  background: rgba(7, 27, 19, 0.94);
}

.message-item.is-error {
  border-color: rgba(248, 113, 113, 0.35);
  background: rgba(33, 11, 18, 0.94);
}

.message-item.is-info {
  border-color: rgba(74, 142, 255, 0.28);
}

.message-icon {
  width: 22px;
  height: 22px;
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  color: #4a8eff;
}

.message-item.is-success .message-icon {
  color: #22c55e;
}

.message-item.is-error .message-icon {
  color: #fb7185;
}

.message-icon svg,
.message-close svg {
  width: 20px;
  height: 20px;
}

.message-body {
  flex: 1;
  min-width: 0;
  padding-top: 1px;
  color: rgba(255, 255, 255, 0.94);
  font-size: 14px;
  line-height: 1.5;
}

.message-close {
  flex: 0 0 auto;
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: rgba(255, 255, 255, 0.58);
}

.message-stack-enter-active,
.message-stack-leave-active {
  transition:
    transform 0.22s ease,
    opacity 0.22s ease;
}

.message-stack-enter-from,
.message-stack-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>

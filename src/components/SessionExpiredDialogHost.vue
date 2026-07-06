<script setup>
import {
  closeSessionExpiredDialog,
  sessionExpiredDialog,
} from '../utils/sessionExpired';

const emit = defineEmits(['confirm']);

function confirmRelogin() {
  closeSessionExpiredDialog(true);
  emit('confirm');
}

function cancelRelogin() {
  closeSessionExpiredDialog(false);
}
</script>

<template>
  <teleport to="body">
    <transition name="session-expired-fade">
      <div
        v-if="sessionExpiredDialog.visible"
        class="session-expired-mask"
        role="presentation"
      >
        <section
          class="session-expired-dialog"
          aria-modal="true"
          role="dialog"
          aria-labelledby="session-expired-title"
        >
          <h2 id="session-expired-title">登录已过期</h2>
          <p>{{ sessionExpiredDialog.message }}</p>
          <div class="session-expired-actions">
            <button
              type="button"
              class="session-expired-button is-primary"
              @click="confirmRelogin"
            >
              确定
            </button>
            <button
              type="button"
              class="session-expired-button"
              @click="cancelRelogin"
            >
              取消
            </button>
          </div>
        </section>
      </div>
    </transition>
  </teleport>
</template>

<style scoped>
.session-expired-mask {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.56);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}

.session-expired-dialog {
  width: min(360px, 100%);
  padding: 26px 24px 22px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 16px;
  background: #10182f;
  box-shadow: 0 28px 80px rgba(0, 0, 0, 0.46);
  color: #ffffff;
  text-align: center;
}

.session-expired-dialog h2 {
  margin: 0;
  color: rgba(255, 255, 255, 0.96);
  font-size: 18px;
  font-weight: 800;
  line-height: 1.35;
}

.session-expired-dialog p {
  margin: 14px 0 24px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 15px;
  line-height: 1.6;
}

.session-expired-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.session-expired-button {
  min-width: 0;
  height: 42px;
  padding: 0 16px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.88);
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
  transition:
    background 0.18s ease,
    border-color 0.18s ease,
    transform 0.18s ease;
}

.session-expired-button:hover {
  border-color: rgba(255, 255, 255, 0.26);
  background: rgba(255, 255, 255, 0.13);
}

.session-expired-button:active {
  transform: translateY(1px);
}

.session-expired-button.is-primary {
  border-color: rgba(74, 142, 255, 0.56);
  background: #2f73ff;
  color: #ffffff;
}

.session-expired-button.is-primary:hover {
  border-color: rgba(121, 174, 255, 0.78);
  background: #3d82ff;
}

.session-expired-fade-enter-active,
.session-expired-fade-leave-active {
  transition: opacity 0.18s ease;
}

.session-expired-fade-enter-from,
.session-expired-fade-leave-to {
  opacity: 0;
}
</style>

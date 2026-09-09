<script setup>
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import SessionExpiredDialogHost from './components/SessionExpiredDialogHost.vue';
import SystemMessageHost from './components/SystemMessageHost.vue';
import {
  WINDOWS_RUNTIME_DOWNLOAD_URL,
  WINDOWS_RUNTIME_VERSION,
} from './config/windowsRuntime';

const router = useRouter();
const appBootLoading = ref(false);
const appBootReady = ref(false);
const appBootVisible = ref(false);
const appBootProgress = ref(0);
const appBootStatus = ref('正在检查运行环境...');
const appBootPhase = ref('check');
const appBootDownloadedBytes = ref(0);
const appBootTotalBytes = ref(0);
const appBootError = ref('');
const activeRuntimeDownloadId = ref('');
let unlistenRuntimeProgress = null;

const appBootByteProgress = computed(() => {
  if (appBootPhase.value !== 'download' || !appBootTotalBytes.value) return '';
  return `${formatFileSize(appBootDownloadedBytes.value)} / ${formatFileSize(appBootTotalBytes.value)}`;
});

function formatFileSize(bytes) {
  const value = Math.max(0, Number(bytes) || 0);
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(2)} GB`;
  if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
}

function runtimeErrorMessage(error) {
  const message = String(error?.message || error || '运行环境准备失败');
  if (/HTTP (401|403)/i.test(message)) {
    return '运行环境下载链接已失效，请更新下载地址后重试';
  }
  return message;
}

async function prepareWindowsRuntime() {
  appBootVisible.value = true;
  appBootLoading.value = true;
  appBootError.value = '';
  appBootProgress.value = 0;
  appBootStatus.value = '正在检查运行环境...';
  appBootPhase.value = 'check';
  appBootDownloadedBytes.value = 0;
  appBootTotalBytes.value = 0;

  const downloadId = `windows-runtime-${Date.now()}`;
  activeRuntimeDownloadId.value = downloadId;

  try {
    const result = await invoke('prepare_windows_runtime', {
      downloadUrl: WINDOWS_RUNTIME_DOWNLOAD_URL,
      runtimeVersion: WINDOWS_RUNTIME_VERSION,
      downloadId,
    });
    if (!result?.ready) throw new Error('运行环境校验未通过');
    appBootProgress.value = 100;
    appBootStatus.value = '运行环境准备完成';
    appBootReady.value = true;
    appBootVisible.value = false;
  } catch (error) {
    console.error('[windows-runtime] preparation failed', error);
    appBootError.value = runtimeErrorMessage(error);
    appBootStatus.value = '运行环境准备失败';
  } finally {
    appBootLoading.value = false;
  }
}

onMounted(async () => {
  if (!window.__TAURI_INTERNALS__) {
    appBootReady.value = true;
    return;
  }

  try {
    const runtimeReady = await invoke('is_windows_runtime_ready');
    if (runtimeReady) {
      appBootReady.value = true;
      return;
    }

    appBootVisible.value = true;
    appBootLoading.value = true;
    unlistenRuntimeProgress = await listen(
      'windows-runtime-download-progress',
      (event) => {
        const payload = event.payload || {};
        if (payload.downloadId !== activeRuntimeDownloadId.value) return;
        appBootProgress.value = Math.max(
          0,
          Math.min(100, Number(payload.progress) || 0),
        );
        appBootStatus.value = payload.status || appBootStatus.value;
        appBootPhase.value = payload.phase || '';
        appBootDownloadedBytes.value = Number(payload.downloadedBytes) || 0;
        appBootTotalBytes.value = Number(payload.totalBytes) || 0;
      },
    );
    await prepareWindowsRuntime();
  } catch (error) {
    appBootVisible.value = true;
    appBootLoading.value = false;
    appBootError.value = runtimeErrorMessage(error);
    appBootStatus.value = '运行环境准备失败';
  }
});

onBeforeUnmount(() => {
  unlistenRuntimeProgress?.();
});

function handleLoginSuccess() {
  const redirect =
    typeof router.currentRoute.value.query.redirect === 'string'
      ? router.currentRoute.value.query.redirect
      : '/home';

  router.replace(redirect);
}

function handleLogout() {
  localStorage.removeItem('token');
  localStorage.removeItem('userInfo');
  router.replace('/login');
}

function handleSessionExpiredConfirm() {
  localStorage.clear();
  sessionStorage.clear();
  router.replace('/login');
}

const routeViewListeners = computed(() => {
  if (router.currentRoute.value.name === 'login') {
    return { login: handleLoginSuccess };
  }
  if (router.currentRoute.value.name === 'home') {
    return { logout: handleLogout };
  }
  return {};
});
</script>

<template>
  <Transition name="app-boot-fade">
    <div v-if="appBootVisible && !appBootReady" class="app-boot-overlay">
      <div class="app-boot-panel">
        <div v-if="appBootLoading" class="app-boot-ring"></div>
        <div class="app-boot-title">准备AICut运行环境</div>
        <div class="app-boot-status">{{ appBootStatus }}</div>
        <div class="app-boot-progress-track">
          <div
            class="app-boot-progress-value"
            :style="{ width: `${appBootProgress}%` }"
          ></div>
        </div>
        <div class="app-boot-progress-meta">
          <span>{{ Math.round(appBootProgress) }}%</span>
          <span v-if="appBootByteProgress">{{ appBootByteProgress }}</span>
        </div>
        <div v-if="appBootError" class="app-boot-error">
          {{ appBootError }}
        </div>
        <button
          v-if="appBootError && !appBootLoading"
          type="button"
          class="app-boot-retry"
          @click="prepareWindowsRuntime"
        >
          重新下载
        </button>
      </div>
    </div>
  </Transition>
  <RouterView v-if="appBootReady" v-slot="{ Component }">
    <component :is="Component" v-on="routeViewListeners" />
  </RouterView>
  <SessionExpiredDialogHost @confirm="handleSessionExpiredConfirm" />
  <SystemMessageHost />
</template>

<style>
* {
  box-sizing: border-box;
}

html,
body,
#app {
  width: 100%;
  min-height: 100%;
  margin: 0;
}

body {
  min-width: 320px;
  color: #ffffff;
  background: #030617;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

button,
input {
  font-family: inherit;
}

.app-boot-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    radial-gradient(
      circle at 50% 42%,
      rgba(74, 142, 255, 0.18),
      transparent 34%
    ),
    #07122a;
  color: #d9e2ff;
}

.app-boot-panel {
  width: min(440px, calc(100vw - 48px));
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  padding: 34px 38px;
  border: 1px solid rgba(125, 166, 255, 0.16);
  border-radius: 18px;
  background: rgba(12, 26, 57, 0.86);
  box-shadow: 0 24px 72px rgba(0, 0, 0, 0.32);
  backdrop-filter: blur(18px);
}

.app-boot-ring {
  width: 52px;
  height: 52px;
  border-radius: 999px;
  border: 3px solid rgba(217, 226, 255, 0.16);
  border-top-color: #4a8eff;
  box-shadow: 0 0 22px rgba(74, 142, 255, 0.28);
  animation: appBootSpin 0.8s linear infinite;
}

.app-boot-title {
  margin-top: 2px;
  font-size: 17px;
  font-weight: 800;
  color: #f2f6ff;
}

.app-boot-status {
  min-height: 20px;
  font-size: 13px;
  color: rgba(217, 226, 255, 0.72);
  text-align: center;
}

.app-boot-progress-track {
  width: 100%;
  height: 8px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
}

.app-boot-progress-value {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #397cff, #69a3ff);
  transition: width 0.16s linear;
}

.app-boot-progress-meta {
  width: 100%;
  display: flex;
  justify-content: space-between;
  min-height: 18px;
  font-size: 12px;
  color: rgba(217, 226, 255, 0.56);
}

.app-boot-error {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid rgba(255, 111, 111, 0.25);
  border-radius: 9px;
  background: rgba(255, 75, 75, 0.08);
  color: #ffb1b1;
  font-size: 12px;
  line-height: 1.5;
  text-align: center;
  word-break: break-word;
}

.app-boot-retry {
  min-width: 112px;
  height: 36px;
  padding: 0 20px;
  border: 0;
  border-radius: 9px;
  color: #ffffff;
  background: #397cff;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.app-boot-retry:hover {
  background: #4b89ff;
}

.app-boot-fade-leave-active {
  transition: opacity 0.2s ease;
}

.app-boot-fade-leave-to {
  opacity: 0;
}

@keyframes appBootSpin {
  to {
    transform: rotate(360deg);
  }
}

::-webkit-scrollbar {
  width: 6px;
}

::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.1);
}

::-webkit-scrollbar-thumb {
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.2);
}
</style>

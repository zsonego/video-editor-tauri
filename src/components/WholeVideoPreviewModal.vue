<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import AppIcon from './AppIcon.vue';

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  loading: {
    type: Boolean,
    default: false,
  },
  progress: {
    type: Number,
    default: 0,
  },
  status: {
    type: String,
    default: '正在准备预览...',
  },
  error: {
    type: String,
    default: '',
  },
  source: {
    type: String,
    default: '',
  },
  title: {
    type: String,
    default: '整片视频预览',
  },
  autoplay: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(['close']);

const videoRef = ref(null);
const playerRef = ref(null);
const paused = ref(true);
const playbackRate = ref(1);
const currentTime = ref(0);
const duration = ref(0);
const volume = ref(1);
const muted = ref(false);
const fullscreen = ref(false);
let windowWasFullscreen = false;

const normalizedProgress = computed(() =>
  Math.round(Math.max(0, Math.min(100, Number(props.progress) || 0))),
);

function formatTime(value) {
  const secondsValue = Number(value);
  if (!Number.isFinite(secondsValue) || secondsValue < 0) return '00:00';
  const minutes = Math.floor(secondsValue / 60)
    .toString()
    .padStart(2, '0');
  const seconds = Math.floor(secondsValue % 60)
    .toString()
    .padStart(2, '0');
  return `${minutes}:${seconds}`;
}

function resetControls() {
  paused.value = true;
  playbackRate.value = 1;
  currentTime.value = 0;
  duration.value = 0;
}

function releaseVideo() {
  const video = videoRef.value;
  if (video) {
    video.pause();
    video.removeAttribute('src');
    video.load();
  }
  resetControls();
}

function updateControls() {
  const video = videoRef.value;
  if (!video) return;
  paused.value = video.paused;
  currentTime.value = Number.isFinite(video.currentTime)
    ? video.currentTime
    : 0;
  duration.value = Number.isFinite(video.duration) ? video.duration : 0;
  playbackRate.value = video.playbackRate || 1;
  volume.value = video.volume;
  muted.value = video.muted || video.volume === 0;
}

function togglePlayback() {
  const video = videoRef.value;
  if (!video) return;
  if (video.paused) {
    video.play().catch(() => {
      paused.value = true;
    });
  } else {
    video.pause();
  }
  updateControls();
}

function seek(event) {
  const video = videoRef.value;
  if (!video) return;
  const nextTime = Number(event.target.value);
  if (!Number.isFinite(nextTime)) return;
  video.currentTime = Math.max(0, Math.min(duration.value || 0, nextTime));
  updateControls();
}

function cyclePlaybackRate() {
  const video = videoRef.value;
  if (!video) return;
  const rates = [0.5, 1, 1.25, 1.5, 2];
  const currentIndex = rates.indexOf(playbackRate.value);
  const nextRate = rates[(currentIndex + 1) % rates.length];
  video.playbackRate = nextRate;
  playbackRate.value = nextRate;
}

function setVolume(event) {
  const video = videoRef.value;
  if (!video) return;
  const nextVolume = Math.max(
    0,
    Math.min(1, Number(event.target.value) || 0),
  );
  video.volume = nextVolume;
  video.muted = nextVolume === 0;
  updateControls();
}

function toggleMute() {
  const video = videoRef.value;
  if (!video) return;
  video.muted = !video.muted;
  updateControls();
}

async function leaveFullscreen() {
  try {
    if (!windowWasFullscreen) {
      await getCurrentWindow().setFullscreen(false);
    }
  } catch (error) {
    console.warn('[whole-video-preview] failed to leave fullscreen:', error);
  } finally {
    fullscreen.value = false;
    windowWasFullscreen = false;
  }
}

async function toggleFullscreen() {
  if (!playerRef.value) return;
  try {
    if (fullscreen.value) {
      await leaveFullscreen();
      return;
    }
    const appWindow = getCurrentWindow();
    windowWasFullscreen = await appWindow.isFullscreen();
    if (!windowWasFullscreen) {
      await appWindow.setFullscreen(true);
    }
    fullscreen.value = true;
  } catch (error) {
    console.warn('[whole-video-preview] failed to enter fullscreen:', error);
    fullscreen.value = false;
    windowWasFullscreen = false;
  }
}

async function close() {
  if (props.loading) return;
  if (fullscreen.value) await leaveFullscreen();
  releaseVideo();
  emit('close');
}

watch(
  () => props.source,
  async () => {
    resetControls();
    await nextTick();
    const video = videoRef.value;
    if (!video || !props.source) return;
    video.load();
    if (props.autoplay) {
      video.play().catch(() => {});
    }
  },
);

watch(
  () => props.visible,
  (visible) => {
    if (visible) return;
    if (fullscreen.value) void leaveFullscreen();
    releaseVideo();
  },
);

onBeforeUnmount(() => {
  if (fullscreen.value) void leaveFullscreen();
  releaseVideo();
});
</script>

<template>
  <Teleport to="body">
    <Transition name="whole-preview-modal">
      <div
        v-if="visible"
        class="fixed inset-0 z-[430] flex items-center justify-center"
        :class="fullscreen ? 'p-0' : 'p-6'"
      >
        <div class="absolute inset-0 bg-black/70 backdrop-blur-xl"></div>
        <section
          class="relative w-full overflow-hidden bg-surface-container-highest shadow-2xl whole-preview-pop-in"
          :class="
            fullscreen
              ? 'h-full max-w-none border-0 rounded-none'
              : 'max-w-4xl rounded-2xl border border-white/10'
          "
          role="dialog"
          aria-modal="true"
          :aria-label="title"
        >
          <header
            v-if="!fullscreen"
            class="flex h-14 items-center justify-between border-b border-white/10 px-5"
          >
            <div class="flex min-w-0 items-center gap-2">
              <AppIcon
                name="smart_display"
                :size="21"
                class="text-electric-blue"
              />
              <h3 class="truncate text-[15px] font-black text-white">
                {{ title }}
              </h3>
            </div>
            <button
              class="flex h-8 w-8 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
              type="button"
              aria-label="关闭预览"
              :disabled="loading"
              @click="close"
            >
              <AppIcon name="close" :size="20" />
            </button>
          </header>

          <div
            v-if="!source"
            class="flex aspect-video flex-col items-center justify-center gap-6 bg-black/80 px-8 text-center"
          >
            <div
              class="whole-preview-progress"
              :style="{ '--progress': `${normalizedProgress}%` }"
            >
              <div class="absolute inset-0 flex items-center justify-center">
                <span class="text-2xl font-black text-electric-blue">
                  {{ normalizedProgress }}%
                </span>
              </div>
            </div>
            <div class="w-full max-w-md space-y-3">
              <p class="break-words text-sm font-bold text-white">
                {{ status }}
              </p>
              <p v-if="error" class="break-words text-xs text-red-300">
                {{ error }}
              </p>
              <div class="h-1.5 w-full overflow-hidden rounded-full bg-white/10">
                <div
                  class="h-full bg-electric-blue transition-all duration-300"
                  :style="{ width: `${normalizedProgress}%` }"
                ></div>
              </div>
              <button
                v-if="!loading"
                class="mt-3 rounded-lg bg-white/10 px-8 py-2 text-sm font-bold text-white transition-colors hover:bg-white/15"
                type="button"
                @click="close"
              >
                关闭
              </button>
            </div>
          </div>

          <div
            v-else
            ref="playerRef"
            class="group relative bg-black"
            :class="fullscreen ? 'h-full' : 'aspect-video'"
          >
            <video
              ref="videoRef"
              class="h-full w-full bg-black object-contain"
              :src="source"
              disablepictureinpicture
              playsinline
              preload="metadata"
              @click="togglePlayback"
              @loadedmetadata="updateControls"
              @timeupdate="updateControls"
              @play="updateControls"
              @pause="updateControls"
              @ended="updateControls"
              @ratechange="updateControls"
              @volumechange="updateControls"
            ></video>
            <div
              class="pointer-events-none absolute inset-x-0 bottom-0 translate-y-2 bg-gradient-to-t from-black/95 via-black/70 to-transparent px-5 pb-4 pt-10 opacity-0 transition-all duration-200 group-hover:pointer-events-auto group-hover:translate-y-0 group-hover:opacity-100 focus-within:pointer-events-auto focus-within:translate-y-0 focus-within:opacity-100"
            >
              <input
                class="mb-3 h-1.5 w-full cursor-pointer accent-[#4a8eff]"
                type="range"
                min="0"
                :max="duration || 0"
                step="0.01"
                :value="currentTime"
                aria-label="预览播放进度"
                @input="seek"
              />
              <div class="flex items-center gap-3 text-white">
                <button
                  class="flex h-9 w-9 items-center justify-center rounded-full bg-electric-blue text-white transition-all hover:brightness-110 active:scale-95"
                  type="button"
                  :aria-label="paused ? '播放' : '暂停'"
                  @click="togglePlayback"
                >
                  <AppIcon
                    :name="paused ? 'play_arrow' : 'pause'"
                    :size="21"
                    filled
                  />
                </button>
                <span class="text-[11px] tabular-nums text-white/75">
                  {{ formatTime(currentTime) }} / {{ formatTime(duration) }}
                </span>
                <div class="flex-1"></div>
                <button
                  class="h-8 min-w-14 rounded-md border border-white/15 bg-white/10 px-2 text-[11px] font-bold text-white/85 transition-colors hover:bg-white/15 hover:text-white"
                  type="button"
                  title="切换播放倍速"
                  @click="cyclePlaybackRate"
                >
                  {{ playbackRate }}x
                </button>
                <div class="flex items-center gap-2">
                  <button
                    class="flex h-8 w-8 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
                    type="button"
                    :aria-label="muted ? '打开声音' : '静音'"
                    @click="toggleMute"
                  >
                    <AppIcon
                      :name="muted ? 'volume_off' : 'volume_up'"
                      :size="18"
                    />
                  </button>
                  <input
                    class="h-1.5 w-20 cursor-pointer accent-[#4a8eff]"
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    :value="muted ? 0 : volume"
                    aria-label="预览音量"
                    @input="setVolume"
                  />
                </div>
                <button
                  class="flex h-8 w-8 items-center justify-center rounded-md text-white/80 transition-colors hover:bg-white/10 hover:text-white"
                  type="button"
                  :aria-label="fullscreen ? '退出全屏' : '全屏播放'"
                  @click="toggleFullscreen"
                >
                  <AppIcon
                    :name="fullscreen ? 'fullscreen_exit' : 'fullscreen'"
                    :size="19"
                  />
                </button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.whole-preview-progress {
  --progress: 0%;
  position: relative;
  width: 120px;
  height: 120px;
  border-radius: 50%;
  background:
    radial-gradient(closest-side, #07122a 79%, transparent 80% 100%),
    conic-gradient(#4a8eff var(--progress), rgba(74, 142, 255, 0.1) 0);
  box-shadow: 0 0 20px rgba(74, 142, 255, 0.2);
}

.whole-preview-pop-in {
  animation: wholePreviewPopIn 0.25s cubic-bezier(0.34, 1.3, 0.64, 1) both;
}

.whole-preview-modal-enter-active,
.whole-preview-modal-leave-active {
  transition: opacity 0.18s ease;
}

.whole-preview-modal-enter-from,
.whole-preview-modal-leave-to {
  opacity: 0;
}

@keyframes wholePreviewPopIn {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
</style>

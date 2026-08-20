<script setup>
import {
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from 'vue';
import { Canvas, FabricImage } from 'fabric';

const props = defineProps({
  source: { type: String, default: '' },
  sourceName: { type: String, default: '' },
  initialTransform: { type: Object, default: null },
});

const emit = defineEmits([
  'change',
  'video-loaded',
  'playback-change',
  'timeupdate',
  'error',
]);

const CANVAS_WIDTH = 960;
const CANVAS_HEIGHT = 540;
const canvasElement = ref(null);
const errorMessage = ref('');
const isReady = ref(false);
const isPlaying = ref(false);
const isTransformExpanded = ref(true);
const isBeautyExpanded = ref(true);
const transform = reactive({ x: 480, y: 270, angle: 0, scale: 1 });
const DEFAULT_BEAUTY_SETTINGS = Object.freeze({
  lutStyle: 'none',
  lutIntensity: 100,
  skinTone: 'fair',
  smoothing: 0,
  whitening: 0,
  stabilization: false,
  oneClickBeauty: false,
});
const beauty = reactive({ ...DEFAULT_BEAUTY_SETTINGS });

let fabricCanvas = null;
let videoObject = null;
let videoElement = null;
let animationFrameId = 0;
let sourceRevision = 0;

function round(value, precision = 1) {
  const multiplier = 10 ** precision;
  return Math.round((Number(value) || 0) * multiplier) / multiplier;
}

function getCoverScale(video) {
  if (!video.videoWidth || !video.videoHeight) return 1;
  return Math.max(
    CANVAS_WIDTH / video.videoWidth,
    CANVAS_HEIGHT / video.videoHeight,
  );
}

function emitPlaybackState() {
  emit('playback-change', {
    paused: !videoElement || videoElement.paused,
    currentTime: videoElement?.currentTime || 0,
    duration: Number.isFinite(videoElement?.duration)
      ? videoElement.duration
      : 0,
  });
}

function emitTimeUpdate() {
  if (!videoElement) return;
  emit('timeupdate', {
    currentTime: Number.isFinite(videoElement.currentTime)
      ? videoElement.currentTime
      : 0,
    duration: Number.isFinite(videoElement.duration)
      ? videoElement.duration
      : 0,
    paused: videoElement.paused,
  });
}

function emitTransform() {
  emit('change', {
    x: transform.x,
    y: transform.y,
    angle: transform.angle,
    scale: transform.scale,
    scalePercent: round(transform.scale * 100),
    beauty: { ...beauty },
  });
}

function syncBeautySettings(values) {
  if (!values || typeof values !== 'object') return;
  if (typeof values.lutStyle === 'string') beauty.lutStyle = values.lutStyle;
  if (values.skinTone === 'fair' || values.skinTone === 'healthy') {
    beauty.skinTone = values.skinTone;
  }
  for (const key of ['lutIntensity', 'smoothing', 'whitening']) {
    const value = Number(values[key]);
    if (Number.isFinite(value)) beauty[key] = Math.min(100, Math.max(0, value));
  }
  if (typeof values.stabilization === 'boolean') {
    beauty.stabilization = values.stabilization;
  }
  if (typeof values.oneClickBeauty === 'boolean') {
    beauty.oneClickBeauty = values.oneClickBeauty;
  }
}

function setBeautyPercent(key, event) {
  const value = Number(event.target.value);
  if (!Number.isFinite(value)) return;
  beauty[key] = Math.min(100, Math.max(0, value));
  emitTransform();
}

function setSkinTone(value) {
  beauty.skinTone = value;
  emitTransform();
}

function resetBeauty() {
  Object.assign(beauty, DEFAULT_BEAUTY_SETTINGS);
  emitTransform();
}

function syncTransformFromObject() {
  if (!videoObject) return;
  transform.x = round(videoObject.left);
  transform.y = round(videoObject.top);
  transform.angle = round(videoObject.angle);
  transform.scale = round(videoObject.scaleX, 3);
  emitTransform();
}

function applyTransform(values = {}, shouldEmit = true) {
  if (!videoObject) return;
  if (Number.isFinite(values.x)) videoObject.set('left', values.x);
  if (Number.isFinite(values.y)) videoObject.set('top', values.y);
  if (Number.isFinite(values.angle)) videoObject.set('angle', values.angle);
  if (Number.isFinite(values.scale)) {
    const scale = Math.min(10, Math.max(0.01, values.scale));
    videoObject.set({ scaleX: scale, scaleY: scale });
  }
  videoObject.setCoords();
  fabricCanvas?.setActiveObject(videoObject);
  if (shouldEmit) syncTransformFromObject();
  else {
    transform.x = round(videoObject.left);
    transform.y = round(videoObject.top);
    transform.angle = round(videoObject.angle);
    transform.scale = round(videoObject.scaleX, 3);
  }
  fabricCanvas?.requestRenderAll();
}

function addVideoToCanvas(video, revision) {
  if (!fabricCanvas || video !== videoElement || revision !== sourceRevision) {
    return;
  }

  video.width = video.videoWidth;
  video.height = video.videoHeight;
  const initialScale = getCoverScale(video);
  videoObject = new FabricImage(video, {
    left: CANVAS_WIDTH / 2,
    top: CANVAS_HEIGHT / 2,
    originX: 'center',
    originY: 'center',
    scaleX: initialScale,
    scaleY: initialScale,
    objectCaching: false,
    lockScalingFlip: true,
    hoverCursor: 'grab',
    moveCursor: 'grabbing',
    borderColor: '#4a8eff',
    cornerColor: '#ffffff',
    cornerStrokeColor: '#4a8eff',
    cornerStyle: 'circle',
    transparentCorners: false,
    padding: 2,
  });
  videoObject.setControlsVisibility({
    mt: false,
    mb: false,
    ml: false,
    mr: false,
  });
  fabricCanvas.add(videoObject);
  fabricCanvas.setActiveObject(videoObject);
  videoObject.setCoords();
  isReady.value = true;

  if (props.initialTransform) {
    syncBeautySettings(props.initialTransform.beauty);
    applyTransform(props.initialTransform, false);
  } else syncTransformFromObject();

  fabricCanvas.requestRenderAll();
  emit('video-loaded', {
    name: props.sourceName,
    duration: Number.isFinite(video.duration) ? video.duration : 0,
  });
  emitPlaybackState();
}

function onVideoError() {
  errorMessage.value = '视频无法读取，请尝试浏览器支持的视频格式。';
  isReady.value = false;
  emit('error', errorMessage.value);
}

function renderVideoFrame() {
  if (!videoElement || videoElement.paused || videoElement.ended) return;
  fabricCanvas?.requestRenderAll();
  emitTimeUpdate();
  animationFrameId = requestAnimationFrame(renderVideoFrame);
}

function onVideoPlay() {
  isPlaying.value = true;
  emitPlaybackState();
  renderVideoFrame();
}

function onVideoPause() {
  isPlaying.value = false;
  cancelAnimationFrame(animationFrameId);
  animationFrameId = 0;
  fabricCanvas?.requestRenderAll();
  emitPlaybackState();
}

function disposeVideo() {
  sourceRevision += 1;
  cancelAnimationFrame(animationFrameId);
  animationFrameId = 0;
  if (videoObject && fabricCanvas) fabricCanvas.remove(videoObject);
  videoObject = null;
  if (videoElement) {
    videoElement.pause();
    videoElement.removeAttribute('src');
    videoElement.load();
  }
  videoElement = null;
  isReady.value = false;
  isPlaying.value = false;
}

function loadSource(source) {
  disposeVideo();
  errorMessage.value = '';
  if (!source) {
    fabricCanvas?.requestRenderAll();
    return;
  }

  const revision = sourceRevision;
  const video = document.createElement('video');
  video.src = source;
  video.preload = 'auto';
  video.playsInline = true;
  video.crossOrigin = 'anonymous';
  videoElement = video;
  video.addEventListener(
    'loadedmetadata',
    () => addVideoToCanvas(video, revision),
    { once: true },
  );
  video.addEventListener('loadeddata', () => fabricCanvas?.requestRenderAll(), {
    once: true,
  });
  video.addEventListener('play', onVideoPlay);
  video.addEventListener('pause', onVideoPause);
  video.addEventListener('ended', onVideoPause);
  video.addEventListener('timeupdate', emitTimeUpdate);
  video.addEventListener('error', onVideoError);
  video.load();
}

function setNumericTransform(key, event) {
  if (!videoObject) return;
  const value = Number(event.target.value);
  if (!Number.isFinite(value)) return;
  applyTransform({ [key]: value });
}

function setScalePercent(event) {
  const percent = Number(event.target.value);
  if (!Number.isFinite(percent)) return;
  applyTransform({ scale: percent / 100 });
}

function resetRotation() {
  if (!videoObject) return;
  applyTransform({ angle: 0 });
}

function resetTransform() {
  if (!videoElement || !videoObject) return;
  applyTransform({
    x: CANVAS_WIDTH / 2,
    y: CANVAS_HEIGHT / 2,
    angle: 0,
    scale: getCoverScale(videoElement),
  });
}

async function togglePlayback() {
  if (!videoElement || !isReady.value) return;
  try {
    if (videoElement.paused) await videoElement.play();
    else videoElement.pause();
  } catch {
    errorMessage.value = '浏览器阻止了视频播放，请再次点击播放。';
  }
}

function pause() {
  videoElement?.pause();
}

function seekTo(value) {
  if (!videoElement) return;
  const duration = Number.isFinite(videoElement.duration)
    ? videoElement.duration
    : 0;
  const nextTime = duration
    ? Math.min(duration, Math.max(0, Number(value) || 0))
    : Math.max(0, Number(value) || 0);
  try {
    videoElement.currentTime = nextTime;
    fabricCanvas?.requestRenderAll();
    emitTimeUpdate();
  } catch {
    // 元数据就绪后由父组件再次同步时间。
  }
}

function getCurrentTime() {
  return Number.isFinite(videoElement?.currentTime)
    ? videoElement.currentTime
    : 0;
}

function getTransform() {
  return { ...transform, beauty: { ...beauty } };
}

onMounted(async () => {
  await nextTick();
  fabricCanvas = new Canvas(canvasElement.value, {
    width: CANVAS_WIDTH,
    height: CANVAS_HEIGHT,
    backgroundColor: '#000000',
    defaultCursor: 'grab',
    hoverCursor: 'grab',
    moveCursor: 'grabbing',
    preserveObjectStacking: true,
    selection: false,
  });

  const handleObjectTransform = ({ target }) => {
    if (target === videoObject) syncTransformFromObject();
  };
  fabricCanvas.on('object:moving', handleObjectTransform);
  fabricCanvas.on('object:scaling', handleObjectTransform);
  fabricCanvas.on('object:rotating', handleObjectTransform);
  fabricCanvas.on('object:modified', handleObjectTransform);
  fabricCanvas.on('mouse:wheel', ({ e }) => {
    if (!videoObject) return;
    e.preventDefault();
    e.stopPropagation();
    const nextScale = Math.min(
      10,
      Math.max(0.01, videoObject.scaleX * Math.pow(0.999, e.deltaY)),
    );
    applyTransform({ scale: nextScale });
  });
  loadSource(props.source);
});

watch(
  () => props.source,
  (source) => {
    if (fabricCanvas) loadSource(source);
  },
);

watch(
  () => props.initialTransform,
  (values) => {
    if (!values) return;
    syncBeautySettings(values.beauty);
    if (videoObject) applyTransform(values, false);
  },
  { deep: true },
);

onBeforeUnmount(() => {
  disposeVideo();
  fabricCanvas?.dispose();
  fabricCanvas = null;
});

defineExpose({
  getCurrentTime,
  getTransform,
  pause,
  resetBeauty,
  resetRotation,
  resetTransform,
  seekTo,
  setTransform: applyTransform,
  togglePlayback,
});
</script>

<template>
  <section class="video-transformer">
    <p v-if="errorMessage" class="transform-error" role="alert">
      {{ errorMessage }}
    </p>
    <div class="transform-workspace">
      <div class="canvas-column">
        <div class="canvas-shell" :class="{ empty: !isReady }">
          <canvas ref="canvasElement" />
          <div v-if="!isReady" class="empty-state">
            <strong>{{ source ? '正在加载视频…' : '请选择视频素材' }}</strong>
            <span>拖动改变位置 · 拉动圆点缩放 · 使用顶部控制点旋转</span>
          </div>
          <div v-else class="playback-overlay">
            <button
              class="center-play-button"
              type="button"
              :aria-label="isPlaying ? '暂停' : '播放'"
              @click="togglePlayback"
            >
              <svg v-if="!isPlaying" aria-hidden="true" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
              </svg>
              <svg v-else aria-hidden="true" viewBox="0 0 24 24">
                <path d="M7 5h4v14H7zm6 0h4v14h-4z" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <div v-if="$slots.timeline" class="transform-timeline-slot">
        <slot name="timeline"></slot>
      </div>

      <aside class="properties-panel">
        <div class="properties-titlebar">属性</div>
        <section class="property-section">
          <div class="section-heading">
            <button
              class="section-name"
              type="button"
              :aria-expanded="isTransformExpanded"
              aria-controls="transform-properties"
              @click="isTransformExpanded = !isTransformExpanded"
            >
              <svg
                class="section-chevron"
                :class="{ collapsed: !isTransformExpanded }"
                aria-hidden="true"
                viewBox="0 0 16 16"
              >
                <path d="m4.5 6 3.5 3.5L11.5 6" />
              </svg>
              <span>变换</span>
            </button>
            <button
              class="reset-button"
              type="button"
              title="重置变换"
              aria-label="重置变换"
              :disabled="!isReady"
              @click="resetTransform"
            >
              <svg aria-hidden="true" viewBox="0 0 20 20">
                <path d="M4.7 7.2A6 6 0 1 1 4 11m.7-3.8V3.8m0 3.4H8" />
              </svg>
            </button>
          </div>

          <div
            v-show="isTransformExpanded"
            id="transform-properties"
            class="property-rows"
            :class="{ disabled: !isReady }"
          >
            <div class="property-row position-row">
              <span class="property-label">位置</span>
              <div class="position-fields">
                <label class="compact-field">
                  <small>X</small>
                  <input
                    type="number"
                    step="1"
                    :value="transform.x"
                    :disabled="!isReady"
                    @input="setNumericTransform('x', $event)"
                  />
                </label>
                <label class="compact-field">
                  <small>Y</small>
                  <input
                    type="number"
                    step="1"
                    :value="transform.y"
                    :disabled="!isReady"
                    @input="setNumericTransform('y', $event)"
                  />
                </label>
              </div>
            </div>

            <label class="property-row">
              <span class="property-label">缩放</span>
              <div class="value-field">
                <input
                  type="number"
                  min="1"
                  max="1000"
                  step="0.1"
                  :value="round(transform.scale * 100, 1)"
                  :disabled="!isReady"
                  @input="setScalePercent"
                />
                <em>%</em>
              </div>
            </label>

            <div class="property-row rotation-row">
              <span class="property-label">旋转</span>
              <div class="rotation-control">
                <label class="value-field">
                  <input
                    type="number"
                    step="0.1"
                    :value="transform.angle"
                    :disabled="!isReady"
                    @input="setNumericTransform('angle', $event)"
                  />
                  <em>°</em>
                </label>
                <button
                  class="reset-button"
                  type="button"
                  title="重置旋转"
                  aria-label="重置旋转"
                  :disabled="!isReady"
                  @click="resetRotation"
                >
                  <svg aria-hidden="true" viewBox="0 0 20 20">
                    <path d="M4.7 7.2A6 6 0 1 1 4 11m.7-3.8V3.8m0 3.4H8" />
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </section>

        <section class="property-section">
          <div class="section-heading">
            <button
              class="section-name"
              type="button"
              :aria-expanded="isBeautyExpanded"
              aria-controls="beauty-properties"
              @click="isBeautyExpanded = !isBeautyExpanded"
            >
              <svg
                class="section-chevron"
                :class="{ collapsed: !isBeautyExpanded }"
                aria-hidden="true"
                viewBox="0 0 16 16"
              >
                <path d="m4.5 6 3.5 3.5L11.5 6" />
              </svg>
              <span>美颜</span>
            </button>
            <button
              class="reset-button"
              type="button"
              title="还原美颜参数"
              aria-label="还原美颜参数"
              :disabled="!isReady"
              @click="resetBeauty"
            >
              <svg aria-hidden="true" viewBox="0 0 20 20">
                <path d="M4.7 7.2A6 6 0 1 1 4 11m.7-3.8V3.8m0 3.4H8" />
              </svg>
            </button>
          </div>

          <div
            v-show="isBeautyExpanded"
            id="beauty-properties"
            class="property-rows beauty-rows"
            :class="{ disabled: !isReady }"
          >
            <label class="property-row">
              <span class="property-label">LUT 风格</span>
              <select
                v-model="beauty.lutStyle"
                class="property-select"
                @change="emitTransform"
              >
                <option value="none">无</option>
                <option value="clear">清透</option>
                <option value="warm">暖阳</option>
                <option value="cinematic">电影</option>
                <option value="vintage">复古</option>
                <option value="cool">冷调</option>
              </select>
            </label>

            <div class="property-row">
              <span class="property-label">LUT 强度</span>
              <div class="range-control">
                <input
                  class="effect-slider"
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  :value="beauty.lutIntensity"
                  @input="setBeautyPercent('lutIntensity', $event)"
                />
                <label class="value-field compact-value">
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    :value="beauty.lutIntensity"
                    @input="setBeautyPercent('lutIntensity', $event)"
                  />
                  <em>%</em>
                </label>
              </div>
            </div>

            <div class="property-row">
              <span class="property-label">肤色</span>
              <div class="skin-tone-control" role="group" aria-label="肤色选择">
                <button
                  type="button"
                  :class="{ active: beauty.skinTone === 'fair' }"
                  @click="setSkinTone('fair')"
                >
                  白皙
                </button>
                <button
                  type="button"
                  :class="{ active: beauty.skinTone === 'healthy' }"
                  @click="setSkinTone('healthy')"
                >
                  原生
                </button>
              </div>
            </div>

            <div class="property-row">
              <span class="property-label">磨皮</span>
              <div class="range-control">
                <input
                  class="effect-slider"
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  :value="beauty.smoothing"
                  @input="setBeautyPercent('smoothing', $event)"
                />
                <label class="value-field compact-value">
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    :value="beauty.smoothing"
                    @input="setBeautyPercent('smoothing', $event)"
                  />
                  <em>%</em>
                </label>
              </div>
            </div>

            <div class="property-row">
              <span class="property-label">美白</span>
              <div class="range-control">
                <input
                  class="effect-slider"
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  :value="beauty.whitening"
                  @input="setBeautyPercent('whitening', $event)"
                />
                <label class="value-field compact-value">
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    :value="beauty.whitening"
                    @input="setBeautyPercent('whitening', $event)"
                  />
                  <em>%</em>
                </label>
              </div>
            </div>

            <label class="property-row stabilization-row">
              <span class="property-label">视频去抖动</span>
              <span class="checkbox-control">
                <input
                  v-model="beauty.stabilization"
                  type="checkbox"
                  @change="emitTransform"
                />
                <span aria-hidden="true"></span>
              </span>
            </label>

            <label class="property-row one-click-beauty-row">
              <span class="property-label">一键美颜</span>
              <span class="checkbox-control">
                <input
                  v-model="beauty.oneClickBeauty"
                  type="checkbox"
                  @change="emitTransform"
                />
                <span aria-hidden="true"></span>
              </span>
            </label>
          </div>
        </section>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.video-transformer {
  box-sizing: border-box;
  height: 100%;
  overflow: hidden;
  border: 1px solid rgba(74, 142, 255, 0.18);
  border-radius: 12px;
  background: rgba(3, 13, 37, 0.92);
}

.transform-error {
  margin: 0;
  padding: 8px 12px;
  color: #fecaca;
  background: rgba(127, 29, 29, 0.24);
  font-size: 11px;
}

.transform-workspace {
  position: relative;
  display: flex;
  height: 100%;
  min-height: 0;
  padding-right: 232px;
  flex-direction: column;
}

.canvas-column {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  padding: 12px;
  background: #030d25;
}

.transform-timeline-slot {
  min-width: 0;
  flex: 0 0 auto;
  padding: 0 12px 12px;
  background: #030d25;
}

.transform-timeline-slot:empty {
  display: none;
}

.transform-timeline-slot :deep(.timeline-settings-track) {
  margin: 0;
}

.canvas-shell {
  position: relative;
  width: auto;
  height: 100%;
  max-width: 100%;
  max-height: 100%;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: #000000;
}

.canvas-shell :deep(.canvas-container) {
  width: 100% !important;
  height: 100% !important;
}

.canvas-shell :deep(.lower-canvas),
.canvas-shell :deep(.upper-canvas) {
  width: 100% !important;
  height: 100% !important;
}

.empty-state {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 6px;
  color: rgba(217, 226, 255, 0.42);
  pointer-events: none;
}

.empty-state strong {
  color: rgba(217, 226, 255, 0.7);
  font-size: 13px;
}
.empty-state span {
  font-size: 10px;
}

.playback-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.2s ease;
}

.canvas-shell:hover .playback-overlay {
  opacity: 1;
}

.center-play-button {
  display: grid;
  width: 56px;
  height: 56px;
  padding: 0;
  place-items: center;
  border: 1px solid rgba(255, 255, 255, 0.22);
  border-radius: 50%;
  color: #ffffff;
  background: rgba(0, 0, 0, 0.48);
  box-shadow: 0 14px 32px rgba(0, 0, 0, 0.34);
  pointer-events: auto;
}

.center-play-button svg {
  width: 30px;
  height: 30px;
  fill: currentColor;
}

.properties-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 232px;
  min-width: 0;
  padding: 0;
  overflow-x: hidden;
  overflow-y: auto;
  border-left: 1px solid #353535;
  color: #d7d7d7;
  background: #1f1f1f;
  overscroll-behavior: contain;
  scrollbar-color: #555555 transparent;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
}

.properties-panel::-webkit-scrollbar {
  width: 6px;
}

.properties-panel::-webkit-scrollbar-track {
  background: transparent;
}

.properties-panel::-webkit-scrollbar-thumb {
  border-radius: 3px;
  background: #555555;
}

.properties-titlebar {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  height: 30px;
  align-items: center;
  padding: 0 10px;
  border-bottom: 1px solid #353535;
  color: #bdbdbd;
  font-size: 11px;
  font-weight: 600;
}

.property-section {
  border-bottom: 1px solid #363636;
}

.section-heading {
  display: flex;
  height: 30px;
  align-items: center;
  justify-content: space-between;
  padding: 0 7px 0 5px;
  border-bottom: 1px solid #303030;
  background: #272727;
}

.section-name {
  display: flex;
  height: 100%;
  min-width: 0;
  flex: 1;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: 0;
  color: #e1e1e1;
  background: transparent;
  font-size: 11px;
  font-weight: 600;
  text-align: left;
  cursor: pointer;
}

.section-name svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: #b8b8b8;
  stroke-width: 1.5;
}

.section-chevron {
  transition: transform 0.15s ease;
}

.section-chevron.collapsed {
  transform: rotate(-90deg);
}

.reset-button {
  display: grid;
  width: 23px;
  height: 23px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 3px;
  color: #a8a8a8;
  background: transparent;
}

.reset-button:hover:not(:disabled) {
  color: #ffffff;
  background: #383838;
}

.reset-button svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.45;
}

.property-rows {
  display: grid;
  gap: 3px;
  padding: 8px 9px 10px;
}

.property-rows.disabled {
  opacity: 0.48;
  pointer-events: none;
}

.property-row {
  display: grid;
  min-height: 29px;
  grid-template-columns: 55px minmax(0, 1fr);
  align-items: center;
  gap: 7px;
}

.property-label {
  overflow: hidden;
  color: #b5b5b5;
  font-size: 10px;
  line-height: 1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.position-fields {
  display: grid;
  min-width: 0;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
}

.rotation-control {
  display: grid;
  min-width: 0;
  grid-template-columns: minmax(0, 1fr) 23px;
  align-items: center;
  gap: 4px;
}

.range-control {
  display: grid;
  min-width: 0;
  grid-template-columns: minmax(36px, 1fr) 50px;
  align-items: center;
  gap: 6px;
}

.property-select {
  width: 100%;
  height: 25px;
  padding: 0 24px 0 7px;
  border: 1px solid #3b3b3b;
  outline: none;
  border-radius: 2px;
  color: #d7d7d7;
  background: #171717;
  font-size: 10px;
}

.property-select:hover,
.property-select:focus {
  border-color: #4a8eff;
}

.effect-slider {
  width: 100%;
  height: 14px;
  margin: 0;
  accent-color: #4a8eff;
  cursor: pointer;
}

.skin-tone-control {
  display: grid;
  grid-template-columns: 1fr 1fr;
  overflow: hidden;
  border: 1px solid #3b3b3b;
  border-radius: 2px;
}

.skin-tone-control button {
  height: 23px;
  padding: 0 5px;
  border: 0;
  color: #999999;
  background: #171717;
  font-size: 10px;
}

.skin-tone-control button + button {
  border-left: 1px solid #3b3b3b;
}

.skin-tone-control button:hover,
.skin-tone-control button.active {
  color: #ffffff;
  background: #365f9e;
}

.checkbox-control {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
  color: #9f9f9f;
  cursor: pointer;
}

.checkbox-control input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.checkbox-control > span {
  position: relative;
  width: 13px;
  height: 13px;
  flex: 0 0 auto;
  border: 1px solid #555555;
  border-radius: 2px;
  background: #171717;
}

.checkbox-control input:checked + span {
  border-color: #4a8eff;
  background: #4a8eff;
}

.checkbox-control input:checked + span::after {
  position: absolute;
  top: 1px;
  left: 4px;
  width: 3px;
  height: 7px;
  border-right: 1.5px solid #ffffff;
  border-bottom: 1.5px solid #ffffff;
  content: '';
  transform: rotate(45deg);
}

.checkbox-control input:focus-visible + span {
  outline: 1px solid #8bb6ff;
  outline-offset: 1px;
}

.compact-field,
.value-field {
  position: relative;
  min-width: 0;
}

.compact-field small {
  position: absolute;
  top: 50%;
  left: 6px;
  z-index: 1;
  color: #777777;
  font-size: 8px;
  transform: translateY(-50%);
  pointer-events: none;
}

.compact-field input,
.value-field input {
  width: 100%;
  height: 25px;
  border: 1px solid #3b3b3b;
  outline: none;
  border-radius: 2px;
  color: #d7d7d7;
  background: #171717;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  appearance: textfield;
  -moz-appearance: textfield;
}

.compact-field input::-webkit-inner-spin-button,
.compact-field input::-webkit-outer-spin-button,
.value-field input::-webkit-inner-spin-button,
.value-field input::-webkit-outer-spin-button {
  margin: 0;
  appearance: none;
  -webkit-appearance: none;
}

.compact-field input {
  padding: 0 4px 0 16px;
}

.value-field input {
  padding: 0 24px 0 7px;
}

.compact-field input:hover:not(:disabled),
.value-field input:hover:not(:disabled) {
  border-color: #505050;
}

.compact-field input:focus,
.value-field input:focus {
  border-color: #4a8eff;
  box-shadow: inset 0 -1px 0 #4a8eff;
}

.value-field em {
  position: absolute;
  top: 50%;
  right: 7px;
  color: #777777;
  font-size: 9px;
  font-style: normal;
  transform: translateY(-50%);
  pointer-events: none;
}

button:disabled,
input:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

@media (max-width: 820px) {
  .transform-workspace {
    padding-right: 200px;
  }
  .properties-panel {
    width: 200px;
  }
}
</style>

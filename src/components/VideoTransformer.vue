<script setup>
import {
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from 'vue';
import { Canvas, Control, FabricImage, controlsUtils } from 'fabric';
import lutManifest from '../../src-tauri/resources/luts/luts.json';

const props = defineProps({
  source: { type: String, default: '' },
  sourceName: { type: String, default: '' },
  initialTransform: { type: Object, default: null },
  previewLoading: { type: Boolean, default: false },
  videoPreviewLoading: { type: Boolean, default: false },
  applyLoading: { type: Boolean, default: false },
  propertiesLocked: { type: Boolean, default: false },
  materialResetLoading: { type: Boolean, default: false },
});

const emit = defineEmits([
  'change',
  'video-loaded',
  'playback-change',
  'timeupdate',
  'error',
  'preview-request',
  'restore-request',
  'apply-request',
  'material-reset-request',
  'layout-change',
]);

const CANVAS_WIDTH = 960;
const CANVAS_HEIGHT = 540;
const CORNER_ROTATION_OFFSET = 15;
const CORNER_ROTATION_HIT_SIZE = 22;
const ROTATE_CURSOR_PATHS = [
  'M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8',
  'M21 3v5h-5',
];
const canvasElement = ref(null);
const errorMessage = ref('');
const isReady = ref(false);
const isPlaying = ref(false);
const propertiesPanelOpen = ref(false);
const isTransformExpanded = ref(true);
const isBeautyExpanded = ref(true);
const materialResetConfirmVisible = ref(false);
const transform = reactive({ x: 480, y: 270, angle: 0, scale: 1 });
const DEFAULT_BEAUTY_SETTINGS = Object.freeze({
  lutStyle: 'none',
  lutIntensity: 50,
  skinTone: 'off',
  skinIntensity: 60,
  smoothing: 0,
  whitening: 0,
  saturation: 100,
  stabilization: false,
  oneClickBeauty: false,
});
const beauty = reactive({ ...DEFAULT_BEAUTY_SETTINGS });
const LUT_OPTIONS = Object.freeze(
  lutManifest.luts.map((lut) => Object.freeze({ ...lut })),
);
const LUT_OPTION_IDS = new Set(LUT_OPTIONS.map((lut) => lut.id));
const SKIN_TONE_OPTIONS = Object.freeze([
  { value: 'off', label: '不设置', color: 'transparent' },
  { value: 'natural', label: '白皙', color: '#fddcbe' },
  { value: 'warm', label: '原生', color: '#d5a273' },
]);

let fabricCanvas = null;
let videoObject = null;
let videoElement = null;
let videoBaseScale = 1;
let animationFrameId = 0;
let sourceRevision = 0;
let beautyPreviewRevision = 0;
let rotationLastVisualAngle = null;
let activeLutStyle = 'none';
const beautyPreviewSource = ref('');
const beautyPreviewVideoSource = ref('');
const beautyPreviewVideoElement = ref(null);

function round(value, precision = 1) {
  const multiplier = 10 ** precision;
  return Math.round((Number(value) || 0) * multiplier) / multiplier;
}

function normalizeVisualAngle(value) {
  const angle = Number(value);
  if (!Number.isFinite(angle)) return 0;
  return ((angle % 360) + 360) % 360;
}

function getShortestAngleDelta(current, previous) {
  let delta = normalizeVisualAngle(current) - normalizeVisualAngle(previous);
  if (delta > 180) delta -= 360;
  if (delta < -180) delta += 360;
  return delta;
}

function getCoverScale(video) {
  if (!video.videoWidth || !video.videoHeight) return 1;
  return Math.max(
    CANVAS_WIDTH / video.videoWidth,
    CANVAS_HEIGHT / video.videoHeight,
  );
}

function createRotationCursor(rotation) {
  const paths = ROTATE_CURSOR_PATHS.map(
    (path) => `<path d="${path}"/>`,
  ).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round"><g transform="rotate(${rotation} 12 12)" stroke="#111827" stroke-width="4">${paths}</g><g transform="rotate(${rotation} 12 12)" stroke="#f8fafc" stroke-width="2">${paths}</g></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") 10 10, crosshair`;
}

function createCornerRotationControl(x, y, cursorRotation) {
  return new Control({
    x,
    y,
    offsetX: x < 0 ? -CORNER_ROTATION_OFFSET : CORNER_ROTATION_OFFSET,
    offsetY: y < 0 ? -CORNER_ROTATION_OFFSET : CORNER_ROTATION_OFFSET,
    sizeX: CORNER_ROTATION_HIT_SIZE,
    sizeY: CORNER_ROTATION_HIT_SIZE,
    touchSizeX: CORNER_ROTATION_HIT_SIZE + 8,
    touchSizeY: CORNER_ROTATION_HIT_SIZE + 8,
    actionName: 'rotate',
    actionHandler: controlsUtils.rotationWithSnapping,
    cursorStyle: createRotationCursor(cursorRotation),
    cursorStyleHandler: controlsUtils.rotationStyleHandler,
    render: () => {},
  });
}

function addCornerRotationControls(target) {
  target.controls = {
    ...target.controls,
    rotateTl: createCornerRotationControl(-0.5, -0.5, -90),
    rotateTr: createCornerRotationControl(0.5, -0.5, 0),
    rotateBr: createCornerRotationControl(0.5, 0.5, 90),
    rotateBl: createCornerRotationControl(-0.5, 0.5, 180),
  };
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

function getBeautySettings() {
  const selectedLut = LUT_OPTIONS.find((lut) => lut.id === beauty.lutStyle);
  return {
    ...beauty,
    lutFile: selectedLut?.file || '',
  };
}

function emitTransform(changeType = 'transform') {
  if (changeType !== 'beauty') clearBeautyPreview();
  emit('change', {
    changeType,
    x: transform.x,
    y: transform.y,
    angle: transform.angle,
    scale: transform.scale,
    scalePercent: round(transform.scale * 100),
    canvasWidth: CANVAS_WIDTH,
    canvasHeight: CANVAS_HEIGHT,
    transformOrigin: 'center',
    beauty: getBeautySettings(),
  });
}

function syncBeautySettings(values) {
  if (!values || typeof values !== 'object') return;
  if (values.lutStyle === 'none' || LUT_OPTION_IDS.has(values.lutStyle)) {
    beauty.lutStyle = values.lutStyle;
    activeLutStyle = values.lutStyle;
  }
  if (SKIN_TONE_OPTIONS.some((option) => option.value === values.skinTone)) {
    beauty.skinTone = values.skinTone;
  } else if (values.skinTone === 'healthy') {
    beauty.skinTone = 'natural';
  }
  for (const key of [
    'lutIntensity',
    'skinIntensity',
    'smoothing',
    'whitening',
    'saturation',
  ]) {
    const value = Number(values[key]);
    const max = key === 'saturation' ? 200 : 100;
    if (Number.isFinite(value)) beauty[key] = Math.min(max, Math.max(0, value));
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
  const max = key === 'saturation' ? 200 : 100;
  beauty[key] = Math.min(max, Math.max(0, value));
  if (key === 'lutIntensity' && LUT_OPTION_IDS.has(activeLutStyle)) {
    beauty.lutStyle = activeLutStyle;
  }
  emitTransform('beauty');
}

function setLutStyle(event) {
  const lutStyle = String(event?.target?.value || 'none');
  activeLutStyle =
    lutStyle === 'none' || LUT_OPTION_IDS.has(lutStyle) ? lutStyle : 'none';
  beauty.lutStyle = activeLutStyle;
  emitTransform('beauty');
}

function setSkinTone(value) {
  beauty.skinTone = value;
  emitTransform('beauty');
}

function resetBeauty() {
  if (props.propertiesLocked) return;
  Object.assign(beauty, DEFAULT_BEAUTY_SETTINGS);
  activeLutStyle = DEFAULT_BEAUTY_SETTINGS.lutStyle;
  emitTransform('beauty');
}

function restoreProperties(values = null) {
  if (!videoElement || !videoObject || props.propertiesLocked) return false;
  clearBeautyPreview();
  Object.assign(beauty, DEFAULT_BEAUTY_SETTINGS);
  activeLutStyle = DEFAULT_BEAUTY_SETTINGS.lutStyle;
  if (values?.beauty) syncBeautySettings(values.beauty);
  applyTransform(
    values || {
      x: CANVAS_WIDTH / 2,
      y: CANVAS_HEIGHT / 2,
      angle: 0,
      scale: 1,
    },
    false,
  );
  emitTransform('beauty');
  return true;
}

function syncTransformFromObject(syncAngle = false) {
  if (!videoObject) return;
  transform.x = round(videoObject.left);
  transform.y = round(videoObject.top);
  if (syncAngle) transform.angle = round(videoObject.angle, 2);
  transform.scale = round(videoObject.scaleX / videoBaseScale, 3);
  emitTransform();
}

function syncContinuousRotation(target) {
  if (target !== videoObject) return;
  const visualAngle = normalizeVisualAngle(target.angle);
  const previousVisualAngle =
    rotationLastVisualAngle ?? normalizeVisualAngle(transform.angle);
  transform.angle = round(
    transform.angle + getShortestAngleDelta(visualAngle, previousVisualAngle),
    2,
  );
  rotationLastVisualAngle = visualAngle;
  syncTransformFromObject();
}

function applyTransform(values = {}, shouldEmit = true) {
  if (!videoObject) return;
  if (Number.isFinite(values.x)) videoObject.set('left', values.x);
  if (Number.isFinite(values.y)) videoObject.set('top', values.y);
  if (Number.isFinite(values.angle)) {
    transform.angle = round(values.angle, 2);
    videoObject.set('angle', normalizeVisualAngle(values.angle));
  }
  if (Number.isFinite(values.scale)) {
    const scale = Math.min(10, Math.max(0.01, values.scale));
    const objectScale = videoBaseScale * scale;
    videoObject.set({ scaleX: objectScale, scaleY: objectScale });
  }
  videoObject.setCoords();
  fabricCanvas?.setActiveObject(videoObject);
  if (shouldEmit) syncTransformFromObject();
  else {
    transform.x = round(videoObject.left);
    transform.y = round(videoObject.top);
    transform.scale = round(videoObject.scaleX / videoBaseScale, 3);
  }
  fabricCanvas?.requestRenderAll();
}

function syncVideoInteractivity() {
  if (!videoObject) return;
  const editable = !props.propertiesLocked;
  videoObject.set({
    selectable: editable,
    evented: editable,
    hasControls: editable,
    hasBorders: editable,
  });
  if (editable) fabricCanvas?.setActiveObject(videoObject);
  else fabricCanvas?.discardActiveObject();
  videoObject.setCoords();
  fabricCanvas?.requestRenderAll();
}

function addVideoToCanvas(video, revision) {
  if (!fabricCanvas || video !== videoElement || revision !== sourceRevision) {
    return;
  }

  video.width = video.videoWidth;
  video.height = video.videoHeight;
  const initialScale = getCoverScale(video);
  videoBaseScale = initialScale;
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
  addCornerRotationControls(videoObject);
  videoObject.setControlsVisibility({
    mt: false,
    mb: false,
    ml: false,
    mr: false,
    mtr: false,
  });
  fabricCanvas.add(videoObject);
  syncVideoInteractivity();
  videoObject.setCoords();
  isReady.value = true;

  if (props.initialTransform) {
    syncBeautySettings(props.initialTransform.beauty);
    applyTransform(props.initialTransform, false);
  } else syncTransformFromObject(true);

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
  rotationLastVisualAngle = null;
  clearBeautyPreview();
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
  videoBaseScale = 1;
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
  if (!videoObject || props.propertiesLocked) return;
  const rawValue = String(event.target.value || '').trim();
  if (!rawValue) return;
  const value = Number(rawValue);
  if (!Number.isFinite(value)) return;
  applyTransform({ [key]: value });
}

function setScalePercent(event) {
  if (props.propertiesLocked) return;
  const percent = Number(event.target.value);
  if (!Number.isFinite(percent)) return;
  applyTransform({ scale: percent / 100 });
}

function resetRotation() {
  if (!videoObject || props.propertiesLocked) return;
  applyTransform({ angle: 0 });
}

function resetTransform() {
  if (!videoElement || !videoObject || props.propertiesLocked) return;
  applyTransform({
    x: CANVAS_WIDTH / 2,
    y: CANVAS_HEIGHT / 2,
    angle: 0,
    scale: 1,
  });
}

function togglePropertiesPanel() {
  propertiesPanelOpen.value = !propertiesPanelOpen.value;
  nextTick(() => {
    fabricCanvas?.requestRenderAll();
    emit('layout-change');
  });
}

function handleWorkspaceTransitionEnd(event) {
  if (
    event.target === event.currentTarget &&
    event.propertyName === 'padding-right'
  ) {
    emit('layout-change');
  }
}

async function togglePlayback() {
  if (!videoElement || !isReady.value) return;
  const previewVideo = beautyPreviewVideoElement.value;
  if (beautyPreviewVideoSource.value && previewVideo) {
    try {
      if (previewVideo.paused) await previewVideo.play();
      else previewVideo.pause();
    } catch {
      errorMessage.value = '浏览器阻止了预览视频播放，请再次点击播放。';
    }
    return;
  }
  clearBeautyPreview();
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
  clearBeautyPreview();
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
  return {
    ...transform,
    beauty: getBeautySettings(),
  };
}

function clearBeautyPreview() {
  beautyPreviewRevision += 1;
  const previewVideo = beautyPreviewVideoElement.value;
  if (previewVideo) {
    previewVideo.pause();
    previewVideo.removeAttribute('src');
    previewVideo.load();
  }
  beautyPreviewSource.value = '';
  beautyPreviewVideoSource.value = '';
  isPlaying.value = videoElement ? !videoElement.paused : false;
}

function showBeautyPreview(source) {
  if (!source || !fabricCanvas || !videoObject) {
    return Promise.reject(new Error('当前视频尚未准备好'));
  }

  const revision = ++beautyPreviewRevision;
  const image = new Image();
  image.crossOrigin = 'anonymous';

  return new Promise((resolve, reject) => {
    image.onload = () => {
      if (
        revision !== beautyPreviewRevision ||
        !fabricCanvas ||
        !videoObject
      ) {
        resolve(false);
        return;
      }

      const previewVideo = beautyPreviewVideoElement.value;
      if (previewVideo) {
        previewVideo.pause();
        previewVideo.removeAttribute('src');
        previewVideo.load();
      }
      beautyPreviewVideoSource.value = '';
      beautyPreviewSource.value = source;
      isPlaying.value = videoElement ? !videoElement.paused : false;
      resolve(true);
    };
    image.onerror = () => reject(new Error('美颜预览图片加载失败'));
    image.src = source;
  });
}

async function showBeautyVideoPreview(source) {
  clearBeautyPreview();
  if (!source || !fabricCanvas || !videoObject) {
    throw new Error('当前视频尚未准备好');
  }

  beautyPreviewVideoSource.value = source;
  await nextTick();
  const previewVideo = beautyPreviewVideoElement.value;
  if (!previewVideo) return false;

  previewVideo.currentTime = 0;
  try {
    await previewVideo.play();
  } catch {
    // 自动播放被限制时保留首帧，用户可以点击画面中央继续播放。
  }
  return true;
}

function requestBeautyPreview() {
  if (!isReady.value || props.previewLoading || props.propertiesLocked) return;
  videoElement?.pause();
  emit('preview-request', getTransform());
}

function requestFooterPrimaryAction() {
  if (beautyPreviewVideoSource.value) {
    emit('apply-request', getTransform());
    return;
  }
  requestBeautyPreview();
}

function confirmMaterialReset() {
  materialResetConfirmVisible.value = false;
  emit('material-reset-request');
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
  fabricCanvas.on('object:rotating', ({ target }) => {
    syncContinuousRotation(target);
  });
  fabricCanvas.on('object:modified', () => {
    rotationLastVisualAngle = null;
  });
  fabricCanvas.on('mouse:wheel', ({ e }) => {
    if (!videoObject || props.propertiesLocked) return;
    e.preventDefault();
    e.stopPropagation();
    const nextScale = Math.min(
      10,
      Math.max(0.01, transform.scale * Math.pow(0.999, e.deltaY)),
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
  () => props.propertiesLocked,
  () => syncVideoInteractivity(),
);

onBeforeUnmount(() => {
  disposeVideo();
  fabricCanvas?.dispose();
  fabricCanvas = null;
});

defineExpose({
  clearBeautyPreview,
  getCurrentTime,
  getTransform,
  pause,
  resetBeauty,
  restoreProperties,
  resetRotation,
  resetTransform,
  seekTo,
  setTransform: applyTransform,
  showBeautyPreview,
  showBeautyVideoPreview,
  togglePlayback,
});
</script>

<template>
  <section class="video-transformer">
    <p v-if="errorMessage" class="transform-error" role="alert">
      {{ errorMessage }}
    </p>
    <div
      class="transform-workspace"
      :class="{ 'properties-open': propertiesPanelOpen }"
      @transitionend="handleWorkspaceTransitionEnd"
    >
      <div class="canvas-column">
        <div class="canvas-shell" :class="{ empty: !isReady }">
          <canvas ref="canvasElement" />
          <img
            v-if="beautyPreviewSource"
            class="beauty-preview-media"
            :src="beautyPreviewSource"
            alt="美颜预览"
          />
          <video
            v-if="beautyPreviewVideoSource"
            ref="beautyPreviewVideoElement"
            class="beauty-preview-media"
            :src="beautyPreviewVideoSource"
            preload="auto"
            playsinline
            @play="isPlaying = true"
            @pause="isPlaying = false"
            @ended="isPlaying = false"
          ></video>
          <div
            v-if="videoPreviewLoading"
            class="beauty-video-loading-overlay"
            role="status"
            aria-live="polite"
          >
            <svg
              class="beauty-video-loading-hourglass"
              aria-hidden="true"
              viewBox="0 0 48 48"
            >
              <path d="M14 7h20M14 41h20" />
              <path d="M16 8c0 8 3.2 11.2 8 16-4.8 4.8-8 8-8 16" />
              <path d="M32 8c0 8-3.2 11.2-8 16 4.8 4.8 8 8 8 16" />
              <path class="hourglass-sand" d="M19 14h10l-5 6zM18 37h12l-6-8z" />
            </svg>
            <span>正在生成视频预览…</span>
          </div>
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
        <button
          class="properties-panel-toggle"
          type="button"
          :aria-expanded="propertiesPanelOpen"
          :aria-label="propertiesPanelOpen ? '收起属性面板' : '展开属性面板'"
          :title="propertiesPanelOpen ? '收起属性面板' : '展开属性面板'"
          @click="togglePropertiesPanel"
        >
          <svg aria-hidden="true" viewBox="0 0 20 20">
            <path
              :d="propertiesPanelOpen ? 'm7 4 6 6-6 6' : 'm13 4-6 6 6 6'"
            />
          </svg>
        </button>
      </div>

      <div v-if="$slots.timeline" class="transform-timeline-slot">
        <slot name="timeline"></slot>
      </div>

      <aside class="properties-panel">
        <div class="properties-titlebar">
          <span>属性</span>
          <button
            class="material-reset-button"
            type="button"
            :disabled="materialResetLoading"
            @click="materialResetConfirmVisible = true"
          >
            {{ materialResetLoading ? '重置中…' : '素材重置' }}
          </button>
        </div>
        <div
          class="properties-content"
          :class="{ 'is-locked': propertiesLocked }"
          :inert="propertiesLocked || undefined"
          :aria-disabled="propertiesLocked"
        >
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
                      step="0.01"
                      :value="transform.angle.toFixed(2)"
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
                  @change="setLutStyle"
                >
                  <option value="none">无</option>
                  <option
                    v-for="lut in LUT_OPTIONS"
                    :key="lut.id"
                    :value="lut.id"
                  >
                    {{ lut.name }}
                  </option>
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

              <div class="skin-tone-settings">
                <div class="skin-tone-heading">
                  <span>肤色</span>
                </div>
                <div
                  class="skin-tone-swatches"
                  role="group"
                  aria-label="肤色选择"
                >
                  <button
                    v-for="option in SKIN_TONE_OPTIONS"
                    :key="option.value"
                    class="skin-tone-swatch"
                    :class="{
                      active: beauty.skinTone === option.value,
                      off: option.value === 'off',
                    }"
                    :style="{ '--swatch-color': option.color }"
                    type="button"
                    :title="option.label"
                    :aria-label="option.label"
                    :aria-pressed="beauty.skinTone === option.value"
                    @click="setSkinTone(option.value)"
                  ></button>
                </div>

                <template v-if="beauty.skinTone !== 'off'">
                  <div class="property-row skin-adjustment-row">
                    <span class="property-label">程度</span>
                    <div class="range-control">
                      <input
                        class="effect-slider"
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        :value="beauty.skinIntensity"
                        @input="setBeautyPercent('skinIntensity', $event)"
                      />
                      <label class="value-field compact-value">
                        <input
                          type="number"
                          min="0"
                          max="100"
                          step="1"
                          :value="beauty.skinIntensity"
                          @input="setBeautyPercent('skinIntensity', $event)"
                        />
                      </label>
                    </div>
                  </div>
                </template>
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

              <div class="property-row">
                <span class="property-label">饱和度</span>
                <div class="range-control">
                  <input
                    class="effect-slider"
                    type="range"
                    min="0"
                    max="200"
                    step="1"
                    :value="beauty.saturation"
                    @input="setBeautyPercent('saturation', $event)"
                  />
                  <label class="value-field compact-value">
                    <input
                      type="number"
                      min="0"
                      max="200"
                      step="1"
                      :value="beauty.saturation"
                      @input="setBeautyPercent('saturation', $event)"
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
                    @change="emitTransform('beauty')"
                  />
                  <span aria-hidden="true"></span>
                </span>
              </label>

              <label v-if="false" class="property-row one-click-beauty-row">
                <span class="property-label">一键美颜</span>
                <span class="checkbox-control">
                  <input
                    v-model="beauty.oneClickBeauty"
                    type="checkbox"
                    @change="emitTransform('beauty')"
                  />
                  <span aria-hidden="true"></span>
                </span>
              </label>
            </div>
          </section>
        </div>
        <div v-if="!propertiesLocked" class="properties-footer">
          <button
            class="property-footer-button"
            type="button"
            :disabled="!isReady || previewLoading || applyLoading"
            @click="emit('restore-request')"
          >
            一键还原
          </button>
          <button
            class="property-footer-button property-footer-button-primary"
            type="button"
            :disabled="!isReady || previewLoading || applyLoading"
            @click="requestFooterPrimaryAction"
          >
            {{ applyLoading ? '应用中…' : beautyPreviewVideoSource ? '应用' : '预览' }}
          </button>
        </div>
      </aside>
    </div>

    <Teleport to="body">
      <div
        v-if="materialResetConfirmVisible"
        class="material-reset-dialog-backdrop"
        @click.self="materialResetConfirmVisible = false"
      >
        <div
          class="material-reset-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="material-reset-dialog-title"
        >
          <div class="material-reset-dialog-header">
            <strong id="material-reset-dialog-title">素材重置</strong>
          </div>
          <p>是否重置素材为初始状态？</p>
          <div class="material-reset-dialog-actions">
            <button
              class="material-reset-dialog-button"
              type="button"
              @click="materialResetConfirmVisible = false"
            >
              否
            </button>
            <button
              class="material-reset-dialog-button is-confirm"
              type="button"
              @click="confirmMaterialReset"
            >
              是
            </button>
          </div>
        </div>
      </div>
    </Teleport>
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
  padding-right: 0;
  flex-direction: column;
  transition: padding-right 0.25s ease;
}

.transform-workspace.properties-open {
  padding-right: 232px;
}

.canvas-column {
  position: relative;
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  padding: 12px;
  background: #030d25;
  container-type: size;
}

.properties-panel-toggle {
  position: absolute;
  top: 50%;
  right: 0;
  z-index: 40;
  display: flex;
  width: 22px;
  height: 48px;
  align-items: center;
  justify-content: center;
  padding: 0;
  transform: translateY(-50%);
  border: 1px solid #454545;
  border-right: 0;
  border-radius: 10px 0 0 10px;
  color: #d7d7d7;
  background: rgba(31, 31, 31, 0.94);
  box-shadow: -4px 0 12px rgba(0, 0, 0, 0.28);
  cursor: pointer;
  transition:
    color 0.2s ease,
    background 0.2s ease;
}

.properties-panel-toggle:hover {
  color: #ffffff;
  background: #303030;
}

.properties-panel-toggle svg {
  width: 11px;
  height: 11px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}

.transform-timeline-slot {
  min-width: 0;
  flex: 0 0 auto;
  padding: 0 8px 6px;
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
  width: min(100%, calc(100cqh * 16 / 9));
  height: auto;
  max-width: 100%;
  max-height: 100%;
  flex: 0 0 auto;
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

.beauty-preview-media {
  position: absolute;
  z-index: 4;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  object-fit: cover;
}

.beauty-video-loading-overlay {
  position: absolute;
  z-index: 7;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: #f4f7ff;
  background: rgba(3, 8, 20, 0.72);
  backdrop-filter: blur(3px);
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.04em;
}

.beauty-video-loading-hourglass {
  width: 46px;
  height: 46px;
  overflow: visible;
  fill: none;
  stroke: #75a9ff;
  stroke-width: 3;
  stroke-linecap: round;
  stroke-linejoin: round;
  filter: drop-shadow(0 0 9px rgba(74, 142, 255, 0.65));
  animation: beauty-hourglass-flip 1.6s ease-in-out infinite;
}

.beauty-video-loading-hourglass .hourglass-sand {
  fill: #75a9ff;
  stroke: none;
  animation: beauty-hourglass-sand 1.6s ease-in-out infinite;
}

@keyframes beauty-hourglass-flip {
  0%,
  42% {
    transform: rotate(0deg);
  }
  58%,
  100% {
    transform: rotate(180deg);
  }
}

@keyframes beauty-hourglass-sand {
  0%,
  35% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
  65%,
  100% {
    opacity: 1;
  }
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
  min-width: 232px;
  display: flex;
  padding: 0;
  overflow: hidden;
  flex-direction: column;
  border-left: 1px solid #353535;
  color: #d7d7d7;
  background: #1f1f1f;
  opacity: 0;
  pointer-events: none;
  transform: translateX(100%);
  transition:
    transform 0.25s ease,
    opacity 0.2s ease;
}

.transform-workspace.properties-open .properties-panel {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(0);
}

.properties-content {
  min-height: 0;
  flex: 1 1 auto;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-color: #555555 transparent;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
}

.properties-content.is-locked {
  opacity: 0.5;
  pointer-events: none;
  user-select: none;
}

.properties-content::-webkit-scrollbar {
  width: 6px;
}

.properties-content::-webkit-scrollbar-track {
  background: transparent;
}

.properties-content::-webkit-scrollbar-thumb {
  border-radius: 3px;
  background: #555555;
}

.properties-titlebar {
  display: flex;
  flex: 0 0 auto;
  height: 30px;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  border-bottom: 1px solid #353535;
  color: #bdbdbd;
  font-size: 11px;
  font-weight: 600;
}

.material-reset-button {
  height: 21px;
  padding: 0 7px;
  border: 1px solid #4a4a4a;
  border-radius: 3px;
  color: #cfcfcf;
  background: #303030;
  font-size: 9px;
  font-weight: 600;
  cursor: pointer;
}

.material-reset-button:hover {
  border-color: #686868;
  color: #ffffff;
  background: #3a3a3a;
}

.material-reset-dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(0, 0, 0, 0.62);
  backdrop-filter: blur(2px);
}

.material-reset-dialog {
  width: min(340px, calc(100vw - 40px));
  overflow: hidden;
  border: 1px solid #454545;
  border-radius: 8px;
  color: #e5e5e5;
  background: #242424;
  box-shadow: 0 22px 60px rgba(0, 0, 0, 0.58);
}

.material-reset-dialog-header {
  display: flex;
  height: 40px;
  align-items: center;
  justify-content: center;
  padding: 0 14px;
  border-bottom: 1px solid #393939;
  font-size: 12px;
}

.material-reset-dialog p {
  margin: 0;
  padding: 22px 16px;
  color: #e0e0e0;
  font-size: 13px;
  text-align: center;
}

.material-reset-dialog-actions {
  display: flex;
  justify-content: center;
  gap: 18px;
  padding: 0 14px 14px;
}

.material-reset-dialog-button {
  min-width: 64px;
  height: 30px;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #d8d8d8;
  background: #303030;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}

.material-reset-dialog-button:hover {
  border-color: #666666;
  color: #ffffff;
  background: #3a3a3a;
}

.material-reset-dialog-button.is-confirm {
  border-color: #4a8eff;
  color: #ffffff;
  background: #3f78ca;
}

.material-reset-dialog-button.is-confirm:hover {
  border-color: #75a9ff;
  background: #4a8eff;
}

.properties-footer {
  display: grid;
  min-height: 50px;
  padding: 9px;
  flex: 0 0 auto;
  grid-template-columns: 1fr 1fr;
  align-items: center;
  gap: 7px;
  border-top: 1px solid #3a3a3a;
  background: #242424;
  box-shadow: 0 -5px 14px rgba(0, 0, 0, 0.18);
}

.property-footer-button {
  height: 30px;
  padding: 0 8px;
  border: 1px solid #4a4a4a;
  border-radius: 3px;
  color: #d2d2d2;
  background: #303030;
  font-size: 10px;
  font-weight: 600;
}

.property-footer-button:hover {
  border-color: #626262;
  color: #ffffff;
  background: #3a3a3a;
}

.property-footer-button:disabled,
.material-reset-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.property-footer-button:disabled:hover {
  border-color: #4a4a4a;
  color: #d2d2d2;
  background: #303030;
}

.property-footer-button-primary {
  border-color: #4a8eff;
  color: #ffffff;
  background: #3f78ca;
}

.property-footer-button-primary:hover {
  border-color: #75a9ff;
  background: #4a8eff;
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
  appearance: none;
  width: 100%;
  height: 2px;
  margin: 0;
  border-radius: 999px;
  outline: none;
  background: #3b3b3b;
  cursor: pointer;
}

.effect-slider::-webkit-slider-thumb {
  appearance: none;
  width: 8px;
  height: 14px;
  border: 0;
  border-radius: 3px;
  background: #f4f4f4;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
}

.effect-slider::-moz-range-thumb {
  width: 8px;
  height: 14px;
  border: 0;
  border-radius: 3px;
  background: #f4f4f4;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
}

.skin-tone-settings {
  display: grid;
  gap: 9px;
  padding: 5px 0 7px;
}

.skin-tone-heading {
  display: flex;
  align-items: center;
  gap: 5px;
  color: #bdbdbd;
  font-size: 10px;
}

.skin-tone-swatches {
  display: flex;
  align-items: center;
  gap: 12px;
}

.skin-tone-swatch {
  position: relative;
  display: grid;
  width: 25px;
  height: 25px;
  padding: 0;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 50%;
  background: transparent;
}

.skin-tone-swatch::before {
  width: 19px;
  height: 19px;
  border-radius: 50%;
  background: var(--swatch-color);
  content: '';
}

.skin-tone-swatch:hover,
.skin-tone-swatch.active {
  border-color: #eeeeee;
  box-shadow: 0 0 0 1px #111111 inset;
}

.skin-tone-swatch.off::before {
  border: 1px solid #4a4a4a;
  background: transparent;
}

.skin-tone-swatch.off::after {
  position: absolute;
  width: 18px;
  height: 1px;
  background: #4a4a4a;
  content: '';
  transform: rotate(45deg);
}

.skin-adjustment-row {
  min-height: 27px;
}

.compact-value input {
  padding-right: 7px;
  text-align: center;
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

</style>

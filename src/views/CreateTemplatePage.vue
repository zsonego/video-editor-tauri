<script setup>
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import {
  AlertTriangle,
  ArrowRight,
  Captions,
  Check,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Clapperboard,
  Download,
  Film,
  Folder,
  FolderPlus,
  Image,
  Layers3,
  LoaderCircle,
  Maximize2,
  Menu,
  MoreHorizontal,
  Music2,
  Pencil,
  Play,
  Plus,
  Search,
  Settings2,
  Sparkles,
  Trash2,
  Upload,
  Video,
  WandSparkles,
  X,
} from '@lucide/vue';
import logoImage from '../assets/logo.png';
import { assetPath, buildXml, generateId, parseXml } from '../utils/xml';

const router = useRouter();

const storedAccountProfile = computed(() => {
  try {
    const userInfo = JSON.parse(localStorage.getItem('userInfo') || 'null') || {};
    return userInfo.user || userInfo.profile || userInfo.sysUser || userInfo;
  } catch {
    return {};
  }
});
const accountTenantName = computed(
  () => storedAccountProfile.value.renterName || '--',
);
const accountDisplayName = computed(
  () => storedAccountProfile.value.phone || '--',
);

function goWorkspaceHome() {
  router.push({ name: 'home' });
}

const transitionEffects = [
  { value: 'Fade', label: '淡入淡出' },
  { value: 'Scale-Transition', label: '缩放' },
  { value: 'Blur', label: '模糊' },
  { value: 'Slide-Left', label: '左滑' },
  { value: 'Slide-Right', label: '右滑' },
  { value: 'Slide-Up', label: '上滑' },
  { value: 'Slide-Down', label: '下滑' },
  { value: 'Flash', label: '闪光' },
];

const mirrorDirections = [
  { value: 'up', label: '向上' },
  { value: 'down', label: '向下' },
  { value: 'left', label: '向左' },
  { value: 'right', label: '向右' },
];

const triptychRotations = [
  { value: 0, label: '不旋转' },
  { value: 90, label: '90 度' },
  { value: -90, label: '-90 度' },
];

const videoStyleOptions = [
  { value: 'cinematic', label: '电影质感' },
  { value: 'clean', label: '简约清新' },
  { value: 'warm', label: '温暖治愈' },
  { value: 'vintage', label: '复古胶片' },
  { value: 'cyberpunk', label: '赛博霓虹' },
  { value: 'documentary', label: '纪实自然' },
  { value: 'energetic', label: '活力动感' },
  { value: 'minimal', label: '极简高级' },
];

const fontOptions = [
  'Songti SC',
  'PingFang SC',
  'Microsoft YaHei',
  'Arial',
  'Helvetica',
];
const DEFAULT_AREA_BEAUTY_SETTINGS = Object.freeze({
  lutStyle: 'none',
  lutIntensity: 100,
  skinTone: 'off',
  skinIntensity: 60,
  smoothing: 0,
  whitening: 0,
  stabilization: false,
  oneClickBeauty: false,
});
const AREA_SKIN_TONE_OPTIONS = Object.freeze([
  { value: 'off', label: '不设置', color: 'transparent' },
  { value: 'natural', label: '白皙', color: '#fddcbe' },
  { value: 'warm', label: '原生', color: '#d5a273' },
]);
const AREA_CANVAS_WIDTH = 1920;
const AREA_CANVAS_HEIGHT = 1080;
const AREA_ASPECT_WIDTH = 16;
const AREA_ASPECT_HEIGHT = 9;
const AREA_MAX_SCALE = Math.min(
  AREA_CANVAS_WIDTH / AREA_ASPECT_WIDTH,
  AREA_CANVAS_HEIGHT / AREA_ASPECT_HEIGHT,
);
const VIDEO_EXTENSIONS = ['mp4', 'mov', 'm4v', 'avi', 'mkv', 'webm'];
const AUDIO_EXTENSIONS = ['mp3', 'wav', 'm4a', 'aac', 'flac', 'ogg'];
const THUMBNAIL_MAX_WIDTH = 640;
const THUMBNAIL_MAX_HEIGHT = 360;
const THUMBNAIL_CAPTURE_SECONDS = 0.2;
const THUMBNAIL_CONCURRENCY = 2;
const DROPPED_AREA_WIDTH = 960;
const DROPPED_AREA_HEIGHT = 540;
const SEQUENCE_TIMELINE_BASE_PIXELS_PER_SECOND = 16;
const SEQUENCE_TRACK_LABEL_WIDTH = 112;
const SEQUENCE_TRACK_CLIP_MIN_WIDTH = 28;
const SEQUENCE_TRACKS_HORIZONTAL_PADDING = 24;
const SEQUENCE_TIMELINE_MIN_LANE_WIDTH = 484;
const SEQUENCE_TIMELINE_TRAILING_DURATION = 10 * 1000;

const createDefaultMediaGroups = () =>
  [
    { name: '人物镜头', mediaType: 'video' },
    { name: '默认音频', mediaType: 'audio' },
    { name: '录音音频', mediaType: 'audio' },
  ].map(({ name, mediaType }) => ({
    id: generateId(),
    name,
    mediaType,
    minDuration: 3000,
    maxDuration: 10000,
    expanded: true,
    assets: [],
  }));

const createInitialModel = () => ({
  id: generateId(),
  clipsId: generateId(),
  name: '我的视频模板',
  duration: 0,
  resolution: '1920*1080',
  videoStyle: 'cinematic',
  progress: 0,
  demoPath: '',
  tracks: {
    background: '',
    overlay: '',
    audioBackground: '',
    recording: '',
  },
  mediaGroups: createDefaultMediaGroups(),
  clips: [],
});

const model = reactive(createInitialModel());
const selectedFilePaths = reactive({
  demo: '',
  background: '',
  overlay: '',
  audioBackground: '',
  recording: '',
});
const selectedClipId = ref('');
const areaDialogOpen = ref(false);
const areaDraft = ref(null);
const areaModalSectionsExpanded = reactive({
  transform: true,
  position: true,
  beauty: true,
});
const areaAssetGroupFilter = ref('all');
const areaCanvasDragOver = ref(false);
const draggedAreaAssetId = ref('');
const subtitleDialogOpen = ref(false);
const subtitleDraft = ref(null);
const subtitleIsNew = ref(false);
const searchKeyword = ref('');
const basicSettingsExpanded = ref(true);
const toast = reactive({
  visible: false,
  message: '',
  type: 'success',
  timer: null,
});
const confirmDialog = reactive({
  open: false,
  title: '',
  message: '',
  confirmText: '确认',
  tone: 'danger',
  action: null,
});
const areaContextMenu = reactive({
  open: false,
  x: 0,
  y: 0,
  areaId: '',
});
let areaInteraction = null;
let subtitleInteraction = null;
let timelineEditSnapshot = null;
let activeThumbnailJobs = 0;
let thumbnailPageDisposed = false;
const thumbnailQueue = [];
const activeThumbnailCancels = new Set();

const selectedClip = computed(
  () => model.clips.find((clip) => clip.id === selectedClipId.value) ?? null,
);

const allAssets = computed(() =>
  model.mediaGroups.flatMap((group) =>
    group.assets.map((asset) => ({
      ...asset,
      groupId: group.id,
      groupName: group.name,
    })),
  ),
);
const allVideoAssets = computed(() =>
  allAssets.value.filter((asset) => asset.mediaType !== 'audio'),
);
const areaAssetGroupTabs = computed(() => [
  {
    id: 'all',
    name: '全部',
    count: allVideoAssets.value.length,
  },
  ...model.mediaGroups
    .filter((group) => !isAudioGroup(group))
    .map((group) => ({
      id: group.id,
      name: group.name,
      count: group.assets.length,
    })),
]);
const areaPickerAssets = computed(() => {
  if (areaAssetGroupFilter.value === 'all') return allVideoAssets.value;
  if (
    !model.mediaGroups.some(
      (group) =>
        group.id === areaAssetGroupFilter.value && !isAudioGroup(group),
    )
  ) {
    return allVideoAssets.value;
  }
  return allVideoAssets.value.filter(
    (asset) => asset.groupId === areaAssetGroupFilter.value,
  );
});

const normalizedSearchKeyword = computed(() =>
  searchKeyword.value.trim().toLowerCase(),
);

const filteredGroups = computed(() => {
  const keyword = normalizedSearchKeyword.value;
  if (!keyword) return model.mediaGroups;
  return model.mediaGroups.filter(
    (group) =>
      group.name.toLowerCase().includes(keyword) ||
      group.assets.some((asset) => asset.name.toLowerCase().includes(keyword)),
  );
});

function visibleAssetsForGroup(group) {
  const keyword = normalizedSearchKeyword.value;
  if (!keyword || group.name.toLowerCase().includes(keyword)) {
    return group.assets;
  }
  return group.assets.filter((asset) =>
    asset.name.toLowerCase().includes(keyword),
  );
}

const configuredAssetCount = computed(() => allAssets.value.length);
const invalidAreaCount = computed(() => {
  const ids = new Set(allAssets.value.map((asset) => asset.id));
  return model.clips.reduce(
    (total, clip) =>
      total + clip.areas.filter((area) => !ids.has(area.assetId)).length,
    0,
  );
});
const sequenceTimelineDuration = computed(() => {
  const clipsEnd = model.clips.reduce(
    (maximum, clip) => {
      const duration = Math.max(0, Number(clip.duration) || 0);
      const end = Math.max(0, Number(clip.starttime) || 0) + duration;
      return Math.max(maximum, end);
    },
    0,
  );
  return Math.max(1, clipsEnd + SEQUENCE_TIMELINE_TRAILING_DURATION);
});
const sequenceTimelinePixelsPerSecond = computed(() =>
  model.clips.reduce((pixelsPerSecond, clip) => {
    const duration = Math.max(0, Number(clip.duration) || 0);
    if (!duration) return pixelsPerSecond;
    return Math.max(
      pixelsPerSecond,
      ((SEQUENCE_TRACK_CLIP_MIN_WIDTH + 4) * 1000) / duration,
    );
  }, SEQUENCE_TIMELINE_BASE_PIXELS_PER_SECOND),
);
const sequenceTimelineLaneWidth = computed(() =>
  Math.max(
    SEQUENCE_TIMELINE_MIN_LANE_WIDTH,
    Math.ceil(
      (sequenceTimelineDuration.value / 1000) *
        sequenceTimelinePixelsPerSecond.value,
    ),
  ),
);
const sequenceTimelineEffectivePixelsPerSecond = computed(
  () =>
    sequenceTimelineLaneWidth.value /
    (sequenceTimelineDuration.value / 1000),
);
const sequenceTracksStyle = computed(() => ({
  width: `${
    SEQUENCE_TRACKS_HORIZONTAL_PADDING +
    SEQUENCE_TRACK_LABEL_WIDTH +
    sequenceTimelineLaneWidth.value
  }px`,
}));
const sequenceTimelineRulerScale = computed(() => {
  const duration = sequenceTimelineDuration.value;
  const targetStep =
    (120 / sequenceTimelineEffectivePixelsPerSecond.value) * 1000;
  const preferredSteps = [
    1000, 2000, 5000, 10000, 15000, 30000, 60000, 120000, 300000,
    600000,
  ];
  const majorStep =
    preferredSteps.find((step) => step >= targetStep) ||
    Math.ceil(targetStep / 600000) * 600000;
  return {
    duration,
    majorStep,
    minorStep: majorStep / 5,
  };
});
const sequenceTimelineMajorTicks = computed(() => {
  const { duration, majorStep } = sequenceTimelineRulerScale.value;
  const ticks = [];
  for (let value = 0; value <= duration; value += majorStep) {
    ticks.push({ value, left: `${(value / duration) * 100}%` });
  }
  if (ticks.at(-1)?.value !== duration) {
    ticks.push({ value: duration, left: '100%' });
  }
  return ticks;
});
const sequenceTimelineMinorTicks = computed(() => {
  const { duration, majorStep, minorStep } = sequenceTimelineRulerScale.value;
  const ticks = [];
  for (let value = minorStep; value < duration; value += minorStep) {
    if (Math.abs(value / majorStep - Math.round(value / majorStep)) > 0.0001) {
      ticks.push({ value, left: `${(value / duration) * 100}%` });
    }
  }
  return ticks;
});
const areaPreviewStyle = computed(() => {
  return areaRectStyle(areaDraft.value);
});
const areaPreviewThumbnailUrl = computed(
  () =>
    allAssets.value.find((asset) => asset.id === areaDraft.value?.assetId)
      ?.thumbnailUrl || '',
);
const areaPreviewImageStyle = computed(() =>
  areaImageTransformStyle(areaDraft.value),
);
const otherAreaPreviews = computed(() => {
  const draftId = areaDraft.value?.id;
  return (selectedClip.value?.areas ?? [])
    .map((area, index) => ({
      id: area.id,
      label: `A${String(index + 1).padStart(2, '0')}`,
      style: areaRectStyle(area),
      imageStyle: areaImageTransformStyle(area),
      isTriptych: Boolean(area.triptychGroupId),
      thumbnailUrl:
        allAssets.value.find((asset) => asset.id === area.assetId)
          ?.thumbnailUrl || '',
    }))
    .filter((preview) => preview.id !== draftId);
});
const subtitlePreviewStyle = computed(() => {
  if (!subtitleDraft.value) return {};
  const x = clampNumber(subtitleDraft.value.x, 0, AREA_CANVAS_WIDTH);
  const y = clampNumber(subtitleDraft.value.y, 0, AREA_CANVAS_HEIGHT);
  const fontSize = clampNumber(subtitleDraft.value.fontSize, 1, 500);
  return {
    left: `${(x / AREA_CANVAS_WIDTH) * 100}%`,
    top: `${(y / AREA_CANVAS_HEIGHT) * 100}%`,
    color: subtitleDraft.value.color || '#ffffff',
    fontFamily: subtitleDraft.value.fontFamily || 'sans-serif',
    fontSize: `${fontSize / 19.2}cqw`,
  };
});
const subtitlePreviewText = computed(
  () => subtitleDraft.value?.defaultText?.trim() || '字幕预览',
);

function replaceModel(next) {
  disposeAssetGroups(model.mediaGroups);
  Object.keys(model).forEach((key) => delete model[key]);
  Object.assign(model, next);
  Object.keys(selectedFilePaths).forEach((key) => {
    selectedFilePaths[key] = '';
  });
  selectedClipId.value = '';
}

function showToast(message, type = 'success') {
  if (toast.timer) window.clearTimeout(toast.timer);
  toast.message = message;
  toast.type = type;
  toast.visible = true;
  toast.timer = window.setTimeout(() => {
    toast.visible = false;
  }, 3200);
}

function askConfirm({ title, message, confirmText = '确认删除', action }) {
  Object.assign(confirmDialog, {
    open: true,
    title,
    message,
    confirmText,
    tone: 'danger',
    action,
  });
}

function confirmAction() {
  const action = confirmDialog.action;
  confirmDialog.open = false;
  confirmDialog.action = null;
  action?.();
}

function createGroup(name = `新建目录 ${model.mediaGroups.length + 1}`) {
  const group = {
    id: generateId(),
    name,
    mediaType: 'video',
    minDuration: 3000,
    maxDuration: 10000,
    expanded: true,
    assets: [],
  };
  model.mediaGroups.unshift(group);
  showToast(`已创建“${name}”`);
  requestAnimationFrame(() => {
    document.querySelector(`[data-group-id="${group.id}"] input`)?.focus();
  });
  return group;
}

function getDefaultGroup() {
  let group = model.mediaGroups.find((item) => item.name === '人物镜头');
  if (!group) group = createGroup('人物镜头');
  return group;
}

function isAudioGroup(group) {
  return (
    group?.mediaType === 'audio' ||
    group?.name === '默认音频' ||
    group?.name === '录音音频'
  );
}

function mediaPathExtension(path = '') {
  return path.split('.').pop()?.toLowerCase() || '';
}

function acceptedMediaPaths(paths, audio) {
  const extensions = audio ? AUDIO_EXTENSIONS : VIDEO_EXTENSIONS;
  const accepted = paths.filter((path) =>
    extensions.includes(mediaPathExtension(path)),
  );
  if (accepted.length !== paths.length) {
    showToast(audio ? '音频目录只能上传音频文件' : '视频目录只能上传视频文件', 'warning');
  }
  return accepted;
}

async function pickMediaPaths({ multiple = false, audio = false } = {}) {
  try {
    const selected = await openDialog({
      multiple,
      filters: [
        {
          name: audio ? '音频文件' : '视频文件',
          extensions: audio ? AUDIO_EXTENSIONS : VIDEO_EXTENSIONS,
        },
      ],
    });
    if (!selected) return [];
    return (Array.isArray(selected) ? selected : [selected]).filter(Boolean);
  } catch (error) {
    showToast(error?.message || '打开系统文件选择器失败', 'error');
    return [];
  }
}

function createVideoThumbnail(sourcePath) {
  return new Promise((resolve) => {
    const video = document.createElement('video');
    const canvas = document.createElement('canvas');
    let settled = false;
    let timeoutId = 0;

    const cleanup = () => {
      window.clearTimeout(timeoutId);
      activeThumbnailCancels.delete(cancel);
      video.pause();
      video.removeAttribute('src');
      video.load();
    };

    const finish = (result = null) => {
      if (settled) {
        if (result?.thumbnailUrl) URL.revokeObjectURL(result.thumbnailUrl);
        return;
      }
      settled = true;
      cleanup();
      resolve(result);
    };

    const cancel = () => finish(null);
    activeThumbnailCancels.add(cancel);

    const captureFrame = () => {
      if (settled || !video.videoWidth || !video.videoHeight) {
        finish(null);
        return;
      }

      const scale = Math.min(
        THUMBNAIL_MAX_WIDTH / video.videoWidth,
        THUMBNAIL_MAX_HEIGHT / video.videoHeight,
        1,
      );
      canvas.width = Math.max(1, Math.round(video.videoWidth * scale));
      canvas.height = Math.max(1, Math.round(video.videoHeight * scale));

      try {
        const context = canvas.getContext('2d', { alpha: false });
        if (!context) {
          finish(null);
          return;
        }
        context.drawImage(video, 0, 0, canvas.width, canvas.height);
        canvas.toBlob(
          (blob) => {
            if (!blob) {
              finish(null);
              return;
            }
            finish({
              thumbnailUrl: URL.createObjectURL(blob),
              durationMs: Number.isFinite(video.duration)
                ? Math.round(video.duration * 1000)
                : 0,
              width: video.videoWidth,
              height: video.videoHeight,
            });
          },
          'image/webp',
          0.8,
        );
      } catch {
        finish(null);
      }
    };

    video.preload = 'auto';
    video.muted = true;
    video.playsInline = true;
    video.crossOrigin = 'anonymous';
    video.addEventListener(
      'loadedmetadata',
      () => {
        const duration = Number.isFinite(video.duration) ? video.duration : 0;
        const captureTime = duration
          ? Math.min(THUMBNAIL_CAPTURE_SECONDS, duration / 2)
          : 0;

        if (captureTime > 0.01) {
          video.addEventListener('seeked', captureFrame, { once: true });
          video.currentTime = captureTime;
          return;
        }

        if (video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
          captureFrame();
        } else {
          video.addEventListener('loadeddata', captureFrame, { once: true });
        }
      },
      { once: true },
    );
    video.addEventListener('error', () => finish(null), { once: true });
    timeoutId = window.setTimeout(() => finish(null), 15000);

    try {
      video.src = convertFileSrc(sourcePath);
      video.load();
    } catch {
      finish(null);
    }
  });
}

function drainThumbnailQueue() {
  if (thumbnailPageDisposed) return;

  while (activeThumbnailJobs < THUMBNAIL_CONCURRENCY && thumbnailQueue.length) {
    const asset = thumbnailQueue.shift();
    if (!asset || asset.disposed) continue;

    activeThumbnailJobs += 1;
    createVideoThumbnail(asset.sourcePath)
      .then((result) => {
        if (asset.disposed || thumbnailPageDisposed) {
          if (result?.thumbnailUrl) URL.revokeObjectURL(result.thumbnailUrl);
          return;
        }
        if (!result) {
          asset.thumbnailStatus = 'error';
          return;
        }
        Object.assign(asset, result, { thumbnailStatus: 'ready' });
      })
      .finally(() => {
        activeThumbnailJobs -= 1;
        drainThumbnailQueue();
      });
  }
}

function enqueueThumbnail(asset) {
  thumbnailQueue.push(asset);
  drainThumbnailQueue();
}

function disposeAsset(asset) {
  if (!asset) return;
  asset.disposed = true;
  if (asset.thumbnailUrl) {
    URL.revokeObjectURL(asset.thumbnailUrl);
    asset.thumbnailUrl = '';
  }
}

function disposeAssetGroups(groups = []) {
  groups.forEach((group) => group.assets?.forEach(disposeAsset));
}

function addMediaPathsToGroup(paths, group) {
  const audio = isAudioGroup(group);
  const uniquePaths = [
    ...new Set(acceptedMediaPaths(paths.filter(Boolean), audio)),
  ];
  if (!uniquePaths.length) return;

  const assets = uniquePaths.map((sourcePath) =>
    reactive({
      id: generateId(),
      name: fileName(sourcePath),
      filepath: assetPath(fileName(sourcePath)),
      sourcePath,
      mediaType: audio ? 'audio' : 'video',
      thumbnailUrl: '',
      thumbnailStatus: audio ? 'unavailable' : 'loading',
      durationMs: 0,
      width: 0,
      height: 0,
      disposed: false,
    }),
  );

  group.assets.push(...assets);
  group.expanded = true;
  if (!audio) assets.forEach(enqueueThumbnail);
  showToast(`已添加 ${assets.length} 个素材到“${group.name}”`);
}

async function uploadToDefault() {
  const paths = await pickMediaPaths({ multiple: true });
  if (paths.length) addMediaPathsToGroup(paths, getDefaultGroup());
}

async function uploadToGroup(group) {
  const paths = await pickMediaPaths({
    multiple: true,
    audio: isAudioGroup(group),
  });
  if (paths.length) addMediaPathsToGroup(paths, group);
}

function clearAssetReferences(assetIds) {
  const ids = new Set(assetIds);
  let count = 0;
  model.clips.forEach((clip) => {
    clip.areas.forEach((area) => {
      if (ids.has(area.assetId)) {
        area.assetId = '';
        count += 1;
      }
    });
  });
  return count;
}

function removeGroup(group) {
  const hasAssets = group.assets.length > 0;
  askConfirm({
    title: hasAssets ? '删除非空目录？' : '删除素材目录？',
    message: hasAssets
      ? `“${group.name}”内有 ${group.assets.length} 个素材。删除后素材会一并移除，片段中的相关素材选择也会被清空。`
      : `“${group.name}”将从素材库中移除。`,
    action: () => {
      const cleared = clearAssetReferences(
        group.assets.map((asset) => asset.id),
      );
      group.assets.forEach(disposeAsset);
      model.mediaGroups.splice(model.mediaGroups.indexOf(group), 1);
      showToast(
        cleared ? `目录已删除，已清空 ${cleared} 处片段引用` : '目录已删除',
        cleared ? 'warning' : 'success',
      );
    },
  });
}

function removeAsset(group, asset) {
  askConfirm({
    title: '删除这个素材？',
    message: `“${asset.name}”将被移除，片段中的相关素材选择会同步清空。`,
    action: () => {
      const cleared = clearAssetReferences([asset.id]);
      disposeAsset(asset);
      group.assets.splice(group.assets.indexOf(asset), 1);
      showToast(
        cleared ? `素材已删除，已清空 ${cleared} 处引用` : '素材已删除',
      );
    },
  });
}

function createClip() {
  const previous = model.clips.at(-1);
  const clip = {
    id: generateId(),
    name: `片段 ${model.clips.length + 1}`,
    materialType: 'variable',
    starttime: previous
      ? Number(previous.starttime) + Number(previous.duration)
      : 0,
    duration: 3000,
    areas: [],
    subtitles: [],
    transition: {
      enabled: false,
      effect: 'Fade',
      duration: 500,
    },
  };
  model.clips.push(clip);
  selectedClipId.value = clip.id;
  showToast('已添加新片段');
}

function removeClip(clip) {
  askConfirm({
    title: '删除这个片段？',
    message: `“${clip.name}”以及其中的 Area、字幕和转场设置都会被删除。`,
    action: () => {
      model.clips.splice(model.clips.indexOf(clip), 1);
      if (selectedClipId.value === clip.id) selectedClipId.value = '';
      showToast('片段已删除');
    },
  });
}

function timelineNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function captureTimelineEdit() {
  timelineEditSnapshot = model.clips.map((clip) => ({
    id: clip.id,
    starttime: timelineNumber(clip.starttime),
    duration: timelineNumber(clip.duration),
  }));
}

function finishTimelineEdit() {
  timelineEditSnapshot = null;
}

function recalculateAllClipStartTimes() {
  let nextStarttime = 0;
  model.clips.forEach((clip) => {
    clip.starttime = nextStarttime;
    nextStarttime += timelineNumber(clip.duration);
  });
}

function propagateClipTimeline(clip) {
  if (!timelineEditSnapshot || clip.starttime === '' || clip.duration === '') {
    return;
  }
  const changedIndex = model.clips.indexOf(clip);
  if (changedIndex < 0) return;

  for (let index = changedIndex + 1; index < model.clips.length; index += 1) {
    const previousBefore = timelineEditSnapshot[index - 1];
    const currentBefore = timelineEditSnapshot[index];
    if (
      !previousBefore ||
      !currentBefore ||
      previousBefore.id !== model.clips[index - 1].id ||
      currentBefore.id !== model.clips[index].id ||
      currentBefore.starttime !==
        previousBefore.starttime + previousBefore.duration
    ) {
      break;
    }
    const previous = model.clips[index - 1];
    model.clips[index].starttime =
      timelineNumber(previous.starttime) + timelineNumber(previous.duration);
  }
}

function updateClipTiming(clip, field, event) {
  const rawValue = event.target.value;
  clip[field] = rawValue === '' ? '' : Number(rawValue);
  propagateClipTimeline(clip);
}

function moveClip(clip, direction) {
  const index = model.clips.indexOf(clip);
  const nextIndex = index + direction;
  if (nextIndex < 0 || nextIndex >= model.clips.length) return;
  finishTimelineEdit();
  model.clips.splice(index, 1);
  model.clips.splice(nextIndex, 0, clip);
  recalculateAllClipStartTimes();
}

function appendArea({
  assetId = '',
  mirror = 'none',
  speed = 1,
  rotate = 0,
  opacity = 1,
  beauty = null,
  isMirrorGenerated = false,
  mirrorSourceAreaId = '',
  mirrorDirection = '',
  mirrorAxis = null,
  mirroredDirections = [],
  isTriptychGenerated = false,
  triptychGroupId = '',
  triptychSourceAreaId = '',
  triptychRole = '',
  isQuadGenerated = false,
  quadGroupId = '',
  quadSourceAreaId = '',
  quadRole = '',
  index = null,
  x = 0,
  y = 0,
  width = 1920,
  height = 1080,
} = {}) {
  if (!selectedClip.value) return;
  const nextLayerIndex =
    Math.max(
      0,
      ...selectedClip.value.areas.map((area) => Number(area.index) || 0),
    ) + 1;
  selectedClip.value.areas.push({
    id: generateId(),
    assetId,
    mirror,
    speed,
    rotate,
    opacity: clampNumber(opacity, 0, 1),
    beauty: createAreaBeautySettings(beauty),
    isMirrorGenerated,
    mirrorSourceAreaId,
    mirrorDirection,
    mirrorAxis,
    mirroredDirections: [...mirroredDirections],
    isTriptychGenerated,
    triptychGroupId,
    triptychSourceAreaId,
    triptychRole,
    isQuadGenerated,
    quadGroupId,
    quadSourceAreaId,
    quadRole,
    index:
      index === null || index === undefined
        ? nextLayerIndex
        : Math.max(0, Math.round(Number(index) || 0)),
    x,
    y,
    width,
    height,
  });
  areaDraft.value = selectedClip.value.areas.at(-1);
  areaDialogOpen.value = true;
  showToast('画面区域已添加');
  return areaDraft.value;
}

function newArea() {
  appendArea();
  areaAssetGroupFilter.value = 'all';
}

function startAreaAssetDrag(event, asset) {
  draggedAreaAssetId.value = asset.id;
  event.dataTransfer.effectAllowed = 'copy';
  event.dataTransfer.setData('application/x-aicut-asset-id', asset.id);
  event.dataTransfer.setData('text/plain', asset.id);
}

function finishAreaAssetDrag() {
  draggedAreaAssetId.value = '';
  areaCanvasDragOver.value = false;
}

function handleAreaCanvasDragOver(event) {
  if (!draggedAreaAssetId.value) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = 'copy';
  areaCanvasDragOver.value = true;
}

function handleAreaCanvasDragLeave(event) {
  if (event.currentTarget.contains(event.relatedTarget)) return;
  areaCanvasDragOver.value = false;
}

function dropAssetOnAreaCanvas(event) {
  event.preventDefault();
  const assetId =
    event.dataTransfer.getData('application/x-aicut-asset-id') ||
    event.dataTransfer.getData('text/plain') ||
    draggedAreaAssetId.value;
  const assetExists = allAssets.value.some((asset) => asset.id === assetId);
  areaCanvasDragOver.value = false;
  draggedAreaAssetId.value = '';
  if (!assetExists) return;

  const bounds = event.currentTarget.getBoundingClientRect();
  if (!bounds.width || !bounds.height) return;
  const canvasX =
    ((event.clientX - bounds.left) / bounds.width) * AREA_CANVAS_WIDTH;
  const canvasY =
    ((event.clientY - bounds.top) / bounds.height) * AREA_CANVAS_HEIGHT;
  const x = Math.round(
    clampNumber(
      canvasX - DROPPED_AREA_WIDTH / 2,
      0,
      AREA_CANVAS_WIDTH - DROPPED_AREA_WIDTH,
    ),
  );
  const y = Math.round(
    clampNumber(
      canvasY - DROPPED_AREA_HEIGHT / 2,
      0,
      AREA_CANVAS_HEIGHT - DROPPED_AREA_HEIGHT,
    ),
  );

  if (areaDraft.value && !areaDraft.value.assetId) {
    areaDraft.value.assetId = assetId;
    showToast('素材已放入当前画面区域');
  } else {
    appendArea({
      assetId,
      x,
      y,
      width: DROPPED_AREA_WIDTH,
      height: DROPPED_AREA_HEIGHT,
    });
  }
}

function editArea(area) {
  area.beauty = createAreaBeautySettings(area.beauty);
  areaDraft.value = area;
  normalizeAreaDraft();
  areaAssetGroupFilter.value = 'all';
  areaDialogOpen.value = true;
}

function selectAreaAsset(assetId) {
  if (!areaDraft.value || isBoundGeneratedArea(areaDraft.value)) return;
  areaDraft.value.assetId = assetId;
}

function selectAreaFromCanvas(areaId) {
  const area = selectedClip.value?.areas.find((item) => item.id === areaId);
  if (area) editArea(area);
}

function closeAreaContextMenu() {
  areaContextMenu.open = false;
  areaContextMenu.areaId = '';
  window.removeEventListener('pointerdown', closeAreaContextMenu);
  window.removeEventListener('blur', closeAreaContextMenu);
  window.removeEventListener('keydown', handleAreaContextMenuKeydown);
}

function handleAreaContextMenuKeydown(event) {
  if (event.key === 'Escape') closeAreaContextMenu();
}

function openAreaContextMenu(event, areaId) {
  closeAreaContextMenu();
  Object.assign(areaContextMenu, {
    open: true,
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 286)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 220)),
    areaId,
  });
  window.addEventListener('pointerdown', closeAreaContextMenu);
  window.addEventListener('blur', closeAreaContextMenu);
  window.addEventListener('keydown', handleAreaContextMenuKeydown);
}

function closeAreaDialog() {
  closeAreaContextMenu();
  areaDialogOpen.value = false;
}

function areaForContextMenu(areaId = areaContextMenu.areaId) {
  return selectedClip.value?.areas.find((area) => area.id === areaId) ?? null;
}

function isBoundGeneratedArea(area) {
  return Boolean(
    area?.isMirrorGenerated ||
      area?.isTriptychGenerated ||
      area?.isQuadGenerated,
  );
}

function isTriptychSourceArea(area) {
  return Boolean(area?.triptychGroupId && area?.triptychRole === 'center');
}

function isQuadSourceArea(area) {
  return Boolean(area?.quadGroupId && area?.quadRole === 'top-left');
}

function isLayoutSourceArea(area) {
  return isTriptychSourceArea(area) || isQuadSourceArea(area);
}

function mirrorAxisForGeometry(geometry, direction) {
  if (direction === 'left') return geometry.x;
  if (direction === 'right') return geometry.x + geometry.width;
  if (direction === 'up') return geometry.y;
  return geometry.y + geometry.height;
}

function mirrorAreaGeometry(area, direction, mirrorAxis = null) {
  const geometry = normalizedAreaGeometry(area);
  const target = { ...geometry };
  if (
    mirrorAxis !== null &&
    mirrorAxis !== '' &&
    Number.isFinite(Number(mirrorAxis))
  ) {
    const axis = Number(mirrorAxis);
    if (direction === 'left' || direction === 'right') {
      target.x = axis * 2 - geometry.x - geometry.width;
    } else {
      target.y = axis * 2 - geometry.y - geometry.height;
    }
    return target;
  }
  if (direction === 'up') target.y = geometry.y - geometry.height;
  if (direction === 'down') target.y = geometry.y + geometry.height;
  if (direction === 'left') target.x = geometry.x - geometry.width;
  if (direction === 'right') target.x = geometry.x + geometry.width;
  return target;
}

function canMirrorArea(areaId, direction) {
  const area = areaForContextMenu(areaId);
  if (
    !area?.assetId ||
    isBoundGeneratedArea(area) ||
    isLayoutSourceArea(area) ||
    area.mirroredDirections?.includes(direction)
  ) {
    return false;
  }
  const target = mirrorAreaGeometry(area, direction);
  return (
    target.x >= 0 &&
    target.y >= 0 &&
    target.x + target.width <= AREA_CANVAS_WIDTH &&
    target.y + target.height <= AREA_CANVAS_HEIGHT
  );
}

function areaMirrorDirectionCreated(areaId, direction) {
  return Boolean(
    areaForContextMenu(areaId)?.mirroredDirections?.includes(direction),
  );
}

function composeAreaMirror(area, direction) {
  const nextMirror =
    direction === 'left' || direction === 'right'
      ? 'horizontal'
      : 'vertical';
  if (area.mirror === 'none' || !area.mirror) {
    return { mirror: nextMirror, rotate: area.rotate };
  }
  if (area.mirror === nextMirror) {
    return { mirror: 'none', rotate: area.rotate };
  }
  const rotation = Number(area.rotate) || 0;
  return {
    mirror: 'none',
    rotate: ((((rotation + 180) % 360) + 540) % 360) - 180,
  };
}

function createMirroredArea(direction) {
  const sourceArea = areaForContextMenu();
  if (!sourceArea || !canMirrorArea(sourceArea.id, direction)) return;
  const sourceGeometry = normalizedAreaGeometry(sourceArea);
  const mirrorAxis = mirrorAxisForGeometry(sourceGeometry, direction);
  const geometry = mirrorAreaGeometry(sourceArea, direction, mirrorAxis);
  const transform = composeAreaMirror(sourceArea, direction);
  const directionLabel =
    mirrorDirections.find((item) => item.value === direction)?.label || '';
  closeAreaContextMenu();
  appendArea({
    assetId: sourceArea.assetId,
    mirror: transform.mirror,
    speed: sourceArea.speed,
    rotate: transform.rotate,
    opacity: sourceArea.opacity ?? 1,
    beauty: sourceArea.beauty,
    isMirrorGenerated: true,
    mirrorSourceAreaId: sourceArea.id,
    mirrorDirection: direction,
    mirrorAxis,
    ...geometry,
  });
  sourceArea.mirroredDirections = [
    ...new Set([...(sourceArea.mirroredDirections ?? []), direction]),
  ];
  showToast(`已创建${directionLabel}镜像区域`);
}

function canCreateTriptych(areaId = areaContextMenu.areaId) {
  const area = areaForContextMenu(areaId);
  return Boolean(
    area?.assetId &&
      !isBoundGeneratedArea(area) &&
      !isLayoutSourceArea(area) &&
      (!area.mirror || area.mirror === 'none') &&
      !(area.mirroredDirections?.length > 0),
  );
}

function createTriptych(rotation) {
  const sourceArea = areaForContextMenu();
  if (!sourceArea || !canCreateTriptych(sourceArea.id)) return;
  const groupId = generateId();
  closeAreaContextMenu();
  Object.assign(sourceArea, {
    x: 640,
    y: 0,
    width: 640,
    height: 1080,
    mirror: 'none',
    rotate: rotation,
    triptychGroupId: groupId,
    triptychRole: 'center',
  });
  appendArea({
    assetId: sourceArea.assetId,
    mirror: 'none',
    speed: sourceArea.speed,
    rotate: rotation,
    opacity: sourceArea.opacity ?? 1,
    beauty: sourceArea.beauty,
    isTriptychGenerated: true,
    triptychGroupId: groupId,
    triptychSourceAreaId: sourceArea.id,
    triptychRole: 'left',
    x: 0,
    y: 0,
    width: 640,
    height: 1080,
  });
  appendArea({
    assetId: sourceArea.assetId,
    mirror: 'none',
    speed: sourceArea.speed,
    rotate: rotation,
    opacity: sourceArea.opacity ?? 1,
    beauty: sourceArea.beauty,
    isTriptychGenerated: true,
    triptychGroupId: groupId,
    triptychSourceAreaId: sourceArea.id,
    triptychRole: 'right',
    x: 1280,
    y: 0,
    width: 640,
    height: 1080,
  });
  areaDraft.value = sourceArea;
  showToast('三分屏区域已创建');
}

function canCreateQuad(areaId = areaContextMenu.areaId) {
  const area = areaForContextMenu(areaId);
  return Boolean(
    area?.assetId &&
      !isBoundGeneratedArea(area) &&
      !isLayoutSourceArea(area) &&
      (!area.mirror || area.mirror === 'none') &&
      !(area.mirroredDirections?.length > 0),
  );
}

function createQuad() {
  const sourceArea = areaForContextMenu();
  if (!sourceArea || !canCreateQuad(sourceArea.id)) return;

  const groupId = generateId();
  const sharedArea = {
    assetId: sourceArea.assetId,
    mirror: sourceArea.mirror || 'none',
    speed: sourceArea.speed,
    rotate: sourceArea.rotate,
    opacity: sourceArea.opacity ?? 1,
    beauty: sourceArea.beauty,
    isQuadGenerated: true,
    quadGroupId: groupId,
    quadSourceAreaId: sourceArea.id,
    width: 960,
    height: 540,
  };
  closeAreaContextMenu();
  Object.assign(sourceArea, {
    x: 0,
    y: 0,
    width: 960,
    height: 540,
    quadGroupId: groupId,
    quadRole: 'top-left',
  });
  appendArea({
    ...sharedArea,
    quadRole: 'top-right',
    x: 960,
    y: 0,
  });
  appendArea({
    ...sharedArea,
    quadRole: 'bottom-left',
    x: 0,
    y: 540,
  });
  appendArea({
    ...sharedArea,
    quadRole: 'bottom-right',
    x: 960,
    y: 540,
  });
  areaDraft.value = sourceArea;
  showToast('四分屏区域已创建');
}

function openAreaManager() {
  const firstArea = selectedClip.value?.areas[0];
  if (firstArea) {
    editArea(firstArea);
    return;
  }
  areaDraft.value = null;
  areaAssetGroupFilter.value = 'all';
  areaDialogOpen.value = true;
}

function removeArea(area) {
  const linkedMirrorCount = area.isMirrorGenerated
    ? 0
    : (selectedClip.value?.areas ?? []).filter(
        (candidate) =>
          candidate.isMirrorGenerated &&
          candidate.mirrorSourceAreaId === area.id,
      ).length;
  const linkedTriptychCount = area.triptychGroupId
    ? (selectedClip.value?.areas ?? []).filter(
        (candidate) => candidate.triptychGroupId === area.triptychGroupId,
      ).length
    : 0;
  const linkedQuadCount = area.quadGroupId
    ? (selectedClip.value?.areas ?? []).filter(
        (candidate) => candidate.quadGroupId === area.quadGroupId,
      ).length
    : 0;
  askConfirm({
    title: '删除画面区域？',
    message: linkedQuadCount
      ? '该区域属于四分屏，四个关联区域会一起删除。'
      : linkedTriptychCount
        ? '该区域属于三分屏，三个关联区域会一起删除。'
        : linkedMirrorCount
          ? `该区域和关联的 ${linkedMirrorCount} 个镜像区域会一起删除。`
          : '该 Area 的素材和画面参数会被移除。',
    action: () => {
      const areas = selectedClip.value?.areas;
      if (!areas) return;
      const removedIndex = areas.indexOf(area);
      if (removedIndex < 0) return;
      if (
        area.isMirrorGenerated &&
        area.mirrorSourceAreaId &&
        area.mirrorDirection
      ) {
        const sourceArea = areas.find(
          (candidate) => candidate.id === area.mirrorSourceAreaId,
        );
        if (sourceArea?.mirroredDirections) {
          sourceArea.mirroredDirections = sourceArea.mirroredDirections.filter(
            (direction) => direction !== area.mirrorDirection,
          );
        }
      }
      const removedIds = new Set([area.id]);
      if (area.triptychGroupId) {
        areas.forEach((candidate) => {
          if (candidate.triptychGroupId === area.triptychGroupId) {
            removedIds.add(candidate.id);
          }
        });
      }
      if (area.quadGroupId) {
        areas.forEach((candidate) => {
          if (candidate.quadGroupId === area.quadGroupId) {
            removedIds.add(candidate.id);
          }
        });
      }
      if (!area.isMirrorGenerated) {
        areas.forEach((candidate) => {
          if (
            candidate.isMirrorGenerated &&
            candidate.mirrorSourceAreaId === area.id
          ) {
            removedIds.add(candidate.id);
          }
        });
      }
      const remainingAreas = areas.filter(
        (candidate) => !removedIds.has(candidate.id),
      );
      areas.splice(0, areas.length, ...remainingAreas);
      if (areaDraft.value && removedIds.has(areaDraft.value.id)) {
        const nextArea = areas[Math.min(removedIndex, areas.length - 1)];
        if (nextArea) editArea(nextArea);
        else {
          areaDraft.value = null;
        }
      }
      showToast('画面区域已删除');
    },
  });
}

function createSubtitle() {
  if (!selectedClip.value) return;
  subtitleDraft.value = {
    id: generateId(),
    timeMode: 'relative',
    time: 0,
    duration: 5000,
    minlen: 2,
    maxlen: 20,
    defaultText: '',
    fontFamily: 'Songti SC',
    fontSize: 60,
    color: '#ffffff',
    x: 960,
    y: 900,
    expanded: true,
  };
  subtitleIsNew.value = true;
  subtitleDialogOpen.value = true;
}

function editSubtitle(subtitle) {
  subtitleDraft.value = { ...subtitle };
  normalizeSubtitleDraftPosition();
  subtitleIsNew.value = false;
  subtitleDialogOpen.value = true;
}

function saveSubtitle() {
  if (!selectedClip.value || !subtitleDraft.value) return;
  normalizeSubtitleDraftPosition();
  if (subtitleIsNew.value) {
    selectedClip.value.subtitles.push({ ...subtitleDraft.value });
  } else {
    const target = selectedClip.value.subtitles.find(
      (subtitle) => subtitle.id === subtitleDraft.value.id,
    );
    if (target) Object.assign(target, subtitleDraft.value);
  }
  subtitleDialogOpen.value = false;
  showToast(subtitleIsNew.value ? '字幕已添加' : '字幕已保存');
}

function removeSubtitle(subtitle) {
  selectedClip.value?.subtitles.splice(
    selectedClip.value.subtitles.indexOf(subtitle),
    1,
  );
}

async function updateTrackFile(track) {
  const [sourcePath] = await pickMediaPaths({
    audio: track === 'audioBackground' || track === 'recording',
  });
  if (!sourcePath) return;
  selectedFilePaths[track] = sourcePath;
  model.tracks[track] = assetPath(fileName(sourcePath));
}

async function updateDemoFile() {
  const [sourcePath] = await pickMediaPaths();
  if (!sourcePath) return;
  selectedFilePaths.demo = sourcePath;
  model.demoPath = assetPath(fileName(sourcePath));
}

function clearTrackFile(track) {
  selectedFilePaths[track] = '';
  model.tracks[track] = '';
}

function clearDemoFile() {
  selectedFilePaths.demo = '';
  model.demoPath = '';
}

function clampNumber(value, min, max) {
  return Math.min(
    max,
    Math.max(min, Number.isFinite(Number(value)) ? Number(value) : min),
  );
}

function normalizedAreaGeometry(area) {
  const width = clampNumber(area?.width, 1, AREA_CANVAS_WIDTH);
  const height = clampNumber(area?.height, 1, AREA_CANVAS_HEIGHT);
  return {
    x: clampNumber(area?.x, 0, AREA_CANVAS_WIDTH - width),
    y: clampNumber(area?.y, 0, AREA_CANVAS_HEIGHT - height),
    width,
    height,
  };
}

function areaRectStyle(area) {
  if (!area) return {};
  const { x, y, width, height } = normalizedAreaGeometry(area);
  return {
    left: `${(x / AREA_CANVAS_WIDTH) * 100}%`,
    top: `${(y / AREA_CANVAS_HEIGHT) * 100}%`,
    width: `${(width / AREA_CANVAS_WIDTH) * 100}%`,
    height: `${(height / AREA_CANVAS_HEIGHT) * 100}%`,
    zIndex: Math.max(0, Math.round(Number(area.index) || 0)) + 1,
  };
}

function areaImageTransformStyle(area) {
  if (!area) return {};
  const scaleX = area.mirror === 'horizontal' ? -1 : 1;
  const scaleY = area.mirror === 'vertical' ? -1 : 1;
  const rotate = Number.isFinite(Number(area.rotate)) ? Number(area.rotate) : 0;
  const opacity = clampNumber(area.opacity ?? 1, 0, 1);
  const normalizedRotation = ((rotate % 360) + 360) % 360;
  if (normalizedRotation === 90 || normalizedRotation === 270) {
    const width = Math.max(1, Number(area.width) || 1);
    const height = Math.max(1, Number(area.height) || 1);
    return {
      inset: 'auto',
      left: '50%',
      top: '50%',
      width: `${(height / width) * 100}%`,
      height: `${(width / height) * 100}%`,
      opacity,
      transform: `translate(-50%, -50%) rotate(${rotate}deg) scale(${scaleX}, ${scaleY})`,
    };
  }
  return {
    objectFit: isTriptychSourceArea(area) || area.triptychGroupId
      ? 'contain'
      : 'cover',
    opacity,
    transform: `rotate(${rotate}deg) scale(${scaleX}, ${scaleY})`,
  };
}

function createAreaBeautySettings(values = {}) {
  const source = values && typeof values === 'object' ? values : {};
  const skinTone = AREA_SKIN_TONE_OPTIONS.some(
    (option) => option.value === source.skinTone,
  )
    ? source.skinTone
    : 'off';
  return {
    lutStyle:
      typeof source.lutStyle === 'string'
        ? source.lutStyle
        : DEFAULT_AREA_BEAUTY_SETTINGS.lutStyle,
    lutIntensity: clampNumber(
      source.lutIntensity ?? DEFAULT_AREA_BEAUTY_SETTINGS.lutIntensity,
      0,
      100,
    ),
    skinTone,
    skinIntensity: clampNumber(
      source.skinIntensity ?? DEFAULT_AREA_BEAUTY_SETTINGS.skinIntensity,
      0,
      100,
    ),
    smoothing: clampNumber(
      source.smoothing ?? DEFAULT_AREA_BEAUTY_SETTINGS.smoothing,
      0,
      100,
    ),
    whitening: clampNumber(
      source.whitening ?? DEFAULT_AREA_BEAUTY_SETTINGS.whitening,
      0,
      100,
    ),
    stabilization: Boolean(source.stabilization),
    oneClickBeauty: Boolean(source.oneClickBeauty),
  };
}

function normalizeAreaBeautyValue(key, min = 0, max = 100) {
  if (!areaDraft.value?.beauty) return;
  areaDraft.value.beauty[key] = clampNumber(
    areaDraft.value.beauty[key],
    min,
    max,
  );
}

function resetAreaBeauty() {
  if (!areaDraft.value || isBoundGeneratedArea(areaDraft.value)) return;
  areaDraft.value.beauty = createAreaBeautySettings();
}

function mirrorAxisForDirection(area, direction) {
  const boundArea = selectedClip.value?.areas.find(
    (candidate) =>
      candidate.isMirrorGenerated &&
      candidate.mirrorSourceAreaId === area.id &&
      candidate.mirrorDirection === direction,
  );
  return boundArea
    ? resolveMirrorAxis(area, boundArea)
    : mirrorAxisForGeometry(normalizedAreaGeometry(area), direction);
}

function areaMirrorBindingBounds(area, width, height) {
  const directions = new Set(area?.mirroredDirections ?? []);
  const triptychSource = isTriptychSourceArea(area);
  const quadSource = isQuadSourceArea(area);
  if (quadSource) {
    return {
      minX: 0,
      maxX: AREA_CANVAS_WIDTH - width * 2,
      minY: 0,
      maxY: AREA_CANVAS_HEIGHT - height * 2,
      horizontalSlots: 2,
      verticalSlots: 2,
    };
  }
  const hasLeft = triptychSource || directions.has('left');
  const hasRight = triptychSource || directions.has('right');
  const hasUp = directions.has('up');
  const hasDown = directions.has('down');
  if (!triptychSource && directions.size) {
    let minX = 0;
    let maxX = AREA_CANVAS_WIDTH - width;
    let minY = 0;
    let maxY = AREA_CANVAS_HEIGHT - height;
    directions.forEach((direction) => {
      const axis = mirrorAxisForDirection(area, direction);
      if (direction === 'left') {
        minX = Math.max(minX, axis);
        maxX = Math.min(maxX, axis * 2 - width);
      }
      if (direction === 'right') {
        minX = Math.max(minX, axis * 2 - AREA_CANVAS_WIDTH);
        maxX = Math.min(maxX, axis - width);
      }
      if (direction === 'up') {
        minY = Math.max(minY, axis);
        maxY = Math.min(maxY, axis * 2 - height);
      }
      if (direction === 'down') {
        minY = Math.max(minY, axis * 2 - AREA_CANVAS_HEIGHT);
        maxY = Math.min(maxY, axis - height);
      }
    });
    return {
      minX,
      maxX,
      minY,
      maxY,
      horizontalSlots: 1 + Number(hasLeft) + Number(hasRight),
      verticalSlots: 1 + Number(hasUp) + Number(hasDown),
    };
  }
  return {
    minX: hasLeft ? width : 0,
    maxX: AREA_CANVAS_WIDTH - width * (hasRight ? 2 : 1),
    minY: hasUp ? height : 0,
    maxY: AREA_CANVAS_HEIGHT - height * (hasDown ? 2 : 1),
    horizontalSlots: 1 + Number(hasLeft) + Number(hasRight),
    verticalSlots: 1 + Number(hasUp) + Number(hasDown),
  };
}

function resolveMirrorAxis(sourceArea, boundArea) {
  const storedAxis = Number(boundArea.mirrorAxis);
  if (
    boundArea.mirrorAxis !== null &&
    boundArea.mirrorAxis !== '' &&
    Number.isFinite(storedAxis)
  ) {
    return storedAxis;
  }

  const source = normalizedAreaGeometry(sourceArea);
  const bound = normalizedAreaGeometry(boundArea);
  let axis;
  if (boundArea.mirrorDirection === 'left') {
    axis = (source.x + bound.x + bound.width) / 2;
  } else if (boundArea.mirrorDirection === 'right') {
    axis = (source.x + source.width + bound.x) / 2;
  } else if (boundArea.mirrorDirection === 'up') {
    axis = (source.y + bound.y + bound.height) / 2;
  } else {
    axis = (source.y + source.height + bound.y) / 2;
  }
  boundArea.mirrorAxis = axis;
  return axis;
}

function syncMirroredAreasForSource(sourceArea, clip = selectedClip.value) {
  if (!sourceArea || isBoundGeneratedArea(sourceArea) || !clip) return;
  clip.areas.forEach((boundArea) => {
    if (
      boundArea.isMirrorGenerated &&
      boundArea.mirrorSourceAreaId === sourceArea.id
    ) {
      const mirrorAxis = resolveMirrorAxis(sourceArea, boundArea);
      const geometry = mirrorAreaGeometry(
        sourceArea,
        boundArea.mirrorDirection,
        mirrorAxis,
      );
      const transform = composeAreaMirror(
        sourceArea,
        boundArea.mirrorDirection,
      );
      Object.assign(boundArea, {
        assetId: sourceArea.assetId,
        speed: sourceArea.speed,
        opacity: sourceArea.opacity ?? 1,
        beauty: createAreaBeautySettings(sourceArea.beauty),
        mirror: transform.mirror,
        rotate: transform.rotate,
        ...geometry,
      });
      return;
    }
    if (
      boundArea.isTriptychGenerated &&
      boundArea.triptychSourceAreaId === sourceArea.id
    ) {
      const geometry = normalizedAreaGeometry(sourceArea);
      Object.assign(boundArea, {
        assetId: sourceArea.assetId,
        speed: sourceArea.speed,
        opacity: sourceArea.opacity ?? 1,
        beauty: createAreaBeautySettings(sourceArea.beauty),
        mirror: sourceArea.mirror || 'none',
        rotate: sourceArea.rotate,
        x:
          boundArea.triptychRole === 'left'
            ? geometry.x - geometry.width
            : geometry.x + geometry.width,
        y: geometry.y,
        width: geometry.width,
        height: geometry.height,
      });
      return;
    }
    if (
      boundArea.isQuadGenerated &&
      boundArea.quadSourceAreaId === sourceArea.id
    ) {
      const geometry = normalizedAreaGeometry(sourceArea);
      const isRight = boundArea.quadRole.endsWith('right');
      const isBottom = boundArea.quadRole.startsWith('bottom');
      Object.assign(boundArea, {
        assetId: sourceArea.assetId,
        speed: sourceArea.speed,
        opacity: sourceArea.opacity ?? 1,
        beauty: createAreaBeautySettings(sourceArea.beauty),
        mirror: sourceArea.mirror || 'none',
        rotate: sourceArea.rotate,
        x: geometry.x + (isRight ? geometry.width : 0),
        y: geometry.y + (isBottom ? geometry.height : 0),
        width: geometry.width,
        height: geometry.height,
      });
    }
  });
}

function syncAllMirroredAreas() {
  model.clips.forEach((clip) => {
    clip.areas
      .filter((area) => !isBoundGeneratedArea(area))
      .forEach((area) => syncMirroredAreasForSource(area, clip));
  });
}

function normalizeAreaDraft(sizeSource = 'width') {
  if (!areaDraft.value || isBoundGeneratedArea(areaDraft.value)) return;
  if (isTriptychSourceArea(areaDraft.value)) {
    const width = Math.round(
      clampNumber(areaDraft.value.width, 1, AREA_CANVAS_WIDTH / 3),
    );
    const height = Math.round(
      clampNumber(areaDraft.value.height, 1, AREA_CANVAS_HEIGHT),
    );
    const bounds = areaMirrorBindingBounds(areaDraft.value, width, height);
    Object.assign(areaDraft.value, {
      x: Math.round(
        clampNumber(areaDraft.value.x, bounds.minX, bounds.maxX),
      ),
      y: Math.round(
        clampNumber(areaDraft.value.y, bounds.minY, bounds.maxY),
      ),
      width,
      height,
    });
    return;
  }
  const rawScale =
    sizeSource === 'height'
      ? Number(areaDraft.value.height) / AREA_ASPECT_HEIGHT
      : Number(areaDraft.value.width) / AREA_ASPECT_WIDTH;
  const directions = new Set(areaDraft.value.mirroredDirections ?? []);
  const horizontalSlots = isQuadSourceArea(areaDraft.value)
    ? 2
    : 1 + Number(directions.has('left')) + Number(directions.has('right'));
  const verticalSlots = isQuadSourceArea(areaDraft.value)
    ? 2
    : 1 + Number(directions.has('up')) + Number(directions.has('down'));
  let mirrorWidthLimit = AREA_CANVAS_WIDTH;
  let mirrorHeightLimit = AREA_CANVAS_HEIGHT;
  directions.forEach((direction) => {
    const axis = mirrorAxisForDirection(areaDraft.value, direction);
    if (direction === 'left' || direction === 'right') {
      mirrorWidthLimit = Math.min(
        mirrorWidthLimit,
        axis,
        AREA_CANVAS_WIDTH - axis,
      );
    } else {
      mirrorHeightLimit = Math.min(
        mirrorHeightLimit,
        axis,
        AREA_CANVAS_HEIGHT - axis,
      );
    }
  });
  const bindingMaxScale = Math.floor(
    Math.min(
      AREA_CANVAS_WIDTH / horizontalSlots / AREA_ASPECT_WIDTH,
      AREA_CANVAS_HEIGHT / verticalSlots / AREA_ASPECT_HEIGHT,
      mirrorWidthLimit / AREA_ASPECT_WIDTH,
      mirrorHeightLimit / AREA_ASPECT_HEIGHT,
    ),
  );
  const scale = Math.round(
    clampNumber(rawScale, 1, Math.min(AREA_MAX_SCALE, bindingMaxScale)),
  );
  const width = scale * AREA_ASPECT_WIDTH;
  const height = scale * AREA_ASPECT_HEIGHT;
  const bounds = areaMirrorBindingBounds(areaDraft.value, width, height);
  Object.assign(areaDraft.value, {
    x: clampNumber(areaDraft.value.x, bounds.minX, bounds.maxX),
    y: clampNumber(areaDraft.value.y, bounds.minY, bounds.maxY),
    width,
    height,
  });
}

function normalizeAreaIndex(area = areaDraft.value) {
  if (!area) return;
  area.index = Math.max(0, Math.round(Number(area.index) || 0));
}

function normalizeAreaOpacity(area = areaDraft.value) {
  if (!area) return;
  area.opacity = clampNumber(area.opacity ?? 1, 0, 1);
}

function stopAreaInteraction(event) {
  if (event && areaInteraction && event.pointerId !== areaInteraction.pointerId)
    return;
  areaInteraction = null;
  window.removeEventListener('pointermove', moveAreaInteraction);
  window.removeEventListener('pointerup', stopAreaInteraction);
  window.removeEventListener('pointercancel', stopAreaInteraction);
}

function startAreaInteraction(event, mode) {
  if (
    !areaDraft.value ||
    isBoundGeneratedArea(areaDraft.value) ||
    event.button !== 0
  ) {
    return;
  }
  const preview = event.currentTarget.closest('.screen-preview');
  const bounds = preview?.getBoundingClientRect();
  if (!bounds?.width || !bounds?.height) return;

  stopAreaInteraction();
  normalizeAreaDraft();
  const geometry = normalizedAreaGeometry(areaDraft.value);
  areaInteraction = {
    pointerId: event.pointerId,
    mode,
    startClientX: event.clientX,
    startClientY: event.clientY,
    scaleX: AREA_CANVAS_WIDTH / bounds.width,
    scaleY: AREA_CANVAS_HEIGHT / bounds.height,
    startLeft: geometry.x,
    startTop: geometry.y,
    startRight: geometry.x + geometry.width,
    startBottom: geometry.y + geometry.height,
  };
  window.addEventListener('pointermove', moveAreaInteraction);
  window.addEventListener('pointerup', stopAreaInteraction);
  window.addEventListener('pointercancel', stopAreaInteraction);
  event.preventDefault();
}

function moveAreaInteraction(event) {
  if (
    !areaInteraction ||
    event.pointerId !== areaInteraction.pointerId ||
    !areaDraft.value
  )
    return;
  const state = areaInteraction;
  const dx = (event.clientX - state.startClientX) * state.scaleX;
  const dy = (event.clientY - state.startClientY) * state.scaleY;

  if (state.mode === 'move') {
    const width = state.startRight - state.startLeft;
    const height = state.startBottom - state.startTop;
    const bounds = areaMirrorBindingBounds(areaDraft.value, width, height);
    areaDraft.value.x = Math.round(
      clampNumber(state.startLeft + dx, bounds.minX, bounds.maxX),
    );
    areaDraft.value.y = Math.round(
      clampNumber(state.startTop + dy, bounds.minY, bounds.maxY),
    );
    event.preventDefault();
    return;
  }

  const anchorX = state.mode.includes('w') ? state.startRight : state.startLeft;
  const anchorY = state.mode.includes('n') ? state.startBottom : state.startTop;
  const rawWidth = state.mode.includes('w')
    ? anchorX - (state.startLeft + dx)
    : state.startRight + dx - anchorX;
  const rawHeight = state.mode.includes('n')
    ? anchorY - (state.startTop + dy)
    : state.startBottom + dy - anchorY;
  if (isTriptychSourceArea(areaDraft.value)) {
    const width = Math.max(1, Math.round(rawWidth));
    const height = Math.max(1, Math.round(rawHeight));
    Object.assign(areaDraft.value, {
      x: state.mode.includes('w') ? anchorX - width : anchorX,
      y: state.mode.includes('n') ? anchorY - height : anchorY,
      width,
      height,
    });
    normalizeAreaDraft();
    event.preventDefault();
    return;
  }
  const startScale = (state.startRight - state.startLeft) / AREA_ASPECT_WIDTH;
  const widthScale = rawWidth / AREA_ASPECT_WIDTH;
  const heightScale = rawHeight / AREA_ASPECT_HEIGHT;
  const requestedScale =
    Math.abs(widthScale - startScale) >= Math.abs(heightScale - startScale)
      ? widthScale
      : heightScale;
  const maxWidth = state.mode.includes('w')
    ? anchorX
    : AREA_CANVAS_WIDTH - anchorX;
  const maxHeight = state.mode.includes('n')
    ? anchorY
    : AREA_CANVAS_HEIGHT - anchorY;
  const maxScale = Math.max(
    1,
    Math.floor(
      Math.min(maxWidth / AREA_ASPECT_WIDTH, maxHeight / AREA_ASPECT_HEIGHT),
    ),
  );
  const scale = Math.round(clampNumber(requestedScale, 1, maxScale));
  const width = scale * AREA_ASPECT_WIDTH;
  const height = scale * AREA_ASPECT_HEIGHT;
  const left = state.mode.includes('w') ? anchorX - width : anchorX;
  const top = state.mode.includes('n') ? anchorY - height : anchorY;

  Object.assign(areaDraft.value, {
    x: left,
    y: top,
    width,
    height,
  });
  normalizeAreaDraft('width');
  event.preventDefault();
}

function normalizeSubtitleDraftPosition() {
  if (!subtitleDraft.value) return;
  subtitleDraft.value.x = Math.round(
    clampNumber(subtitleDraft.value.x, 0, AREA_CANVAS_WIDTH),
  );
  subtitleDraft.value.y = Math.round(
    clampNumber(subtitleDraft.value.y, 0, AREA_CANVAS_HEIGHT),
  );
}

function stopSubtitleInteraction(event) {
  if (
    event &&
    subtitleInteraction &&
    event.pointerId !== subtitleInteraction.pointerId
  ) {
    return;
  }
  subtitleInteraction = null;
  window.removeEventListener('pointermove', moveSubtitleInteraction);
  window.removeEventListener('pointerup', stopSubtitleInteraction);
  window.removeEventListener('pointercancel', stopSubtitleInteraction);
}

function startSubtitleInteraction(event) {
  if (!subtitleDraft.value || event.button !== 0) return;
  const preview = event.currentTarget.closest('.screen-preview');
  const bounds = preview?.getBoundingClientRect();
  if (!bounds?.width || !bounds?.height) return;

  stopSubtitleInteraction();
  normalizeSubtitleDraftPosition();
  subtitleInteraction = {
    pointerId: event.pointerId,
    startClientX: event.clientX,
    startClientY: event.clientY,
    startX: subtitleDraft.value.x,
    startY: subtitleDraft.value.y,
    scaleX: AREA_CANVAS_WIDTH / bounds.width,
    scaleY: AREA_CANVAS_HEIGHT / bounds.height,
  };
  window.addEventListener('pointermove', moveSubtitleInteraction);
  window.addEventListener('pointerup', stopSubtitleInteraction);
  window.addEventListener('pointercancel', stopSubtitleInteraction);
  event.preventDefault();
}

function moveSubtitleInteraction(event) {
  if (
    !subtitleInteraction ||
    event.pointerId !== subtitleInteraction.pointerId ||
    !subtitleDraft.value
  ) {
    return;
  }
  const dx =
    (event.clientX - subtitleInteraction.startClientX) *
    subtitleInteraction.scaleX;
  const dy =
    (event.clientY - subtitleInteraction.startClientY) *
    subtitleInteraction.scaleY;
  subtitleDraft.value.x = Math.round(
    clampNumber(subtitleInteraction.startX + dx, 0, AREA_CANVAS_WIDTH),
  );
  subtitleDraft.value.y = Math.round(
    clampNumber(subtitleInteraction.startY + dy, 0, AREA_CANVAS_HEIGHT),
  );
  event.preventDefault();
}

function validateModel() {
  if (!model.name.trim()) return '请填写模板名称。';
  if (
    !Number.isInteger(Number(model.duration)) ||
    Number(model.duration) <= 0
  ) {
    return '视频总时长需填写为正整数。';
  }
  if (!model.demoPath) return '请先上传模板示例视频。';
  if (!model.tracks.overlay) return '请先上传必需的顶层视频。';

  const assetIds = new Set(allAssets.value.map((asset) => asset.id));
  for (const group of model.mediaGroups) {
    if (!group.name.trim()) return '素材目录名称不能为空。';
    if (
      Number(group.minDuration) < 0 ||
      Number(group.maxDuration) < 0 ||
      Number(group.minDuration) > Number(group.maxDuration)
    ) {
      return `目录“${group.name}”的时长范围不正确。`;
    }
  }

  for (const clip of model.clips) {
    if (!clip.name.trim()) return '片段名称不能为空。';
    if (!clip.areas.length) {
      return `片段“${clip.name}”至少需要添加一个 Area 才能导出。`;
    }
    if (
      Number(clip.starttime) < 0 ||
      !Number.isInteger(Number(clip.starttime))
    ) {
      return `片段“${clip.name}”的开始时间需为非负整数。`;
    }
    if (
      Number(clip.duration) <= 0 ||
      !Number.isInteger(Number(clip.duration))
    ) {
      return `片段“${clip.name}”的时长需为正整数。`;
    }
    for (const area of clip.areas) {
      if (!assetIds.has(area.assetId))
        return `片段“${clip.name}”中有 Area 未选择素材。`;
      if (Number(area.speed) < 0 || Number(area.speed) > 1) {
        return `片段“${clip.name}”的播放速度需在 0–1 之间。`;
      }
      if (Number(area.rotate) < -360 || Number(area.rotate) > 360) {
        return `片段“${clip.name}”的旋转角度需在 -360–360 之间。`;
      }
      if (Number(area.opacity) < 0 || Number(area.opacity) > 1) {
        return `片段“${clip.name}”的透明度需在 0–1 之间。`;
      }
      if (Number(area.width) <= 0 || Number(area.height) <= 0) {
        return `片段“${clip.name}”的 Area 宽高必须大于 0。`;
      }
      if (
        Number(area.width) * AREA_ASPECT_HEIGHT !==
        Number(area.height) * AREA_ASPECT_WIDTH
      ) {
        return `片段“${clip.name}”的 Area 宽高比必须为 16:9。`;
      }
    }
    if (
      clip.transition.enabled &&
      (Number(clip.transition.duration) < 0 ||
        Number(clip.transition.duration) > 2000)
    ) {
      return `片段“${clip.name}”的转场时长需在 0–2000ms 之间。`;
    }
    for (const subtitle of clip.subtitles) {
      if (
        Number(subtitle.minlen) < 0 ||
        Number(subtitle.minlen) > Number(subtitle.maxlen)
      ) {
        return `片段“${clip.name}”的字幕字数范围不正确。`;
      }
      if (Number(subtitle.time) < 0 || Number(subtitle.duration) <= 0) {
        return `片段“${clip.name}”的字幕时间设置不正确。`;
      }
    }
  }
  return '';
}

function exportXml() {
  const error = validateModel();
  if (error) {
    showToast(error, 'error');
    return;
  }
  const xml = buildXml(model);
  const blob = new Blob([xml], { type: 'application/xml;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  const filename = (model.name || 'template').replace(/[\\/:*?"<>|]/g, '_');
  link.href = url;
  link.download = `${filename}.xml`;
  link.click();
  URL.revokeObjectURL(url);
  showToast('XML 已生成并开始下载');
}

async function importXml(event) {
  const [file] = event.target.files ?? [];
  event.target.value = '';
  if (!file) return;
  try {
    const content = await file.text();
    replaceModel(parseXml(content));
    showToast(`已导入 ${file.name}，所有内部 ID 已重新生成`);
  } catch (error) {
    showToast(error.message || 'XML 导入失败。', 'error');
  }
}

function assetName(assetId) {
  return (
    allAssets.value.find((asset) => asset.id === assetId)?.name ||
    '尚未选择素材'
  );
}

function fileName(path) {
  return path?.split(/[\\/]/).pop() || '';
}

function formatSeconds(milliseconds) {
  const total = Math.max(0, Number(milliseconds) || 0) / 1000;
  const minutes = Math.floor(total / 60);
  const seconds = (total % 60).toFixed(total % 1 ? 1 : 0).padStart(2, '0');
  return `${String(minutes).padStart(2, '0')}:${seconds}`;
}

function formatMilliseconds(value) {
  return `${Math.max(0, Number(value) || 0)} ms`;
}

function clipMaterialType(clip) {
  return clip?.materialType === 'fixed' ? 'fixed' : 'variable';
}

function setClipMaterialType(clip, materialType) {
  if (!clip) return;
  clip.materialType = materialType === 'fixed' ? 'fixed' : 'variable';
}

function clipTransitionDescription(clip) {
  if (!clip?.transition?.enabled) return '';
  const effect =
    transitionEffects.find((item) => item.value === clip.transition.effect)
      ?.label || clip.transition.effect || '转场';
  return `${effect} · ${formatMilliseconds(clip.transition.duration)}`;
}

function sequenceClipTitle(clip) {
  const details = [clip.name, formatMilliseconds(clip.duration)];
  return details.join(' · ');
}

function sequenceTransitionStyle(clip) {
  const total = sequenceTimelineDuration.value;
  const boundary = clampNumber(
    (Number(clip.starttime) || 0) + (Number(clip.duration) || 0),
    0,
    total,
  );
  return {
    left: `${(boundary / total) * 100}%`,
    width: '10px',
  };
}

function sequenceClipStyle(clip) {
  const total = sequenceTimelineDuration.value;
  const start = clampNumber(Number(clip.starttime) || 0, 0, total);
  const duration = Math.max(0, Number(clip.duration) || 0);
  const end = clampNumber(start + duration, start, total);
  return {
    left: `${(start / total) * 100}%`,
    width: `${Math.max(((end - start) / total) * 100, 0.8)}%`,
  };
}

function sequenceTrackFileLabel(path) {
  return path ? fileName(path) : '待上传';
}

watch(
  () =>
    model.clips.flatMap((clip) =>
      clip.areas
        .filter((area) => !isBoundGeneratedArea(area))
        .map((area) => [
          area.id,
          area.assetId,
          area.mirror,
          area.speed,
          area.rotate,
          area.opacity,
          area.x,
          area.y,
          area.width,
          area.height,
          area.beauty?.lutStyle,
          area.beauty?.lutIntensity,
          area.beauty?.skinTone,
          area.beauty?.skinIntensity,
          area.beauty?.smoothing,
          area.beauty?.whitening,
          area.beauty?.stabilization,
          area.beauty?.oneClickBeauty,
          ...(area.mirroredDirections ?? []),
        ]),
    ),
  syncAllMirroredAreas,
  { deep: true, flush: 'sync' },
);

onBeforeUnmount(() => {
  thumbnailPageDisposed = true;
  closeAreaContextMenu();
  thumbnailQueue.splice(0).forEach(disposeAsset);
  activeThumbnailCancels.forEach((cancel) => cancel());
  stopAreaInteraction();
  stopSubtitleInteraction();
  disposeAssetGroups(model.mediaGroups);
  if (toast.timer) window.clearTimeout(toast.timer);
});
</script>

<template>
  <header
    class="fixed top-0 left-0 right-0 z-[300] bg-surface-container/80 backdrop-blur-2xl border-b border-primary/20"
  >
    <div
      class="flex items-center px-6 gap-4 overflow-visible no-scrollbar border-b-2 border-white/10 h-16"
    >
      <div class="flex items-center gap-4 shrink-0">
        <img
          alt="艾咔"
          class="h-10 w-auto object-contain cursor-pointer select-none"
          :src="logoImage"
          @click="goWorkspaceHome"
        />
        <h1
          class="font-display text-[26px] font-bold tracking-tight text-on-surface whitespace-nowrap"
        >
          艾咔· <span class="text-electric-blue font-black">专业版</span>
        </h1>
      </div>
      <div
        class="flex items-center gap-1 rounded-full border p-0.5 shrink-0 bg-surface-container-lowest border-outline-variant/30 shadow-inner ml-4"
      >
        <div
          class="flex items-center gap-1.5 px-3 py-1 bg-surface-container-high border border-electric-blue/40 rounded-full shadow-lg shadow-electric-blue/10"
        >
          <span
            class="text-[13px] text-electric-blue font-bold whitespace-nowrap"
            >{{ accountTenantName }}</span
          >
        </div>
        <div class="flex items-center gap-1.5 px-3 py-1 rounded-full">
          <span
            class="text-[13px] text-on-surface-variant font-medium whitespace-nowrap"
            >{{ accountDisplayName }}</span
          >
        </div>
      </div>
      <div class="flex-1"></div>
      <button
        class="h-9 w-24 text-electric-blue shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-high shadow-sm rounded-lg border border-electric-blue/40"
        type="button"
      >
        <span class="text-[13px] font-bold whitespace-nowrap">创建模板</span>
      </button>
      <button
        class="h-9 w-24 text-on-surface-variant hover:text-electric-blue shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 shadow-sm rounded-lg transition-all active:scale-95 hover:bg-surface-container-high border border-outline-variant/20"
        type="button"
        @click="goWorkspaceHome"
      >
        <span class="text-[13px] font-bold whitespace-nowrap">工程库</span>
      </button>
      <button
        class="h-9 w-24 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant hover:text-electric-blue rounded-lg font-bold shadow-sm hover:bg-surface-container-high active:scale-95 transition-all shrink-0 border border-outline-variant/20"
        type="button"
        @click="exportXml"
      >
        <span class="text-[13px] uppercase tracking-wide whitespace-nowrap"
          >导出</span
        >
      </button>
      <button
        class="h-9 w-24 shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant shadow-sm rounded-lg border border-outline-variant/20"
        type="button"
      >
        <span class="text-[13px] font-bold">个人中心</span>
      </button>
    </div>
  </header>

  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <div class="brand-mark"><Clapperboard :size="20" /></div>
        <div>
          <strong>AICut</strong>
          <span>哎咔模板加工厂</span>
        </div>
      </div>

      <div class="topbar-center">
        <span class="status-dot"></span>
        <span>{{ model.clips.length }} 个片段</span>
        <span class="separator"></span>
        <span>{{ configuredAssetCount }} 个素材</span>
      </div>

      <div class="topbar-actions">
        <label class="button button-secondary import-xml-action" hidden>
          <Upload :size="16" />
          导入 XML
          <input
            class="sr-only"
            type="file"
            accept=".xml,text/xml,application/xml"
            @change="importXml"
          />
        </label>
        <button
          class="button button-primary preview-button"
          type="button"
          title="预览功能开发中"
          disabled
        >
          <Play :size="16" />
          预览
        </button>
        <button class="button button-primary" type="button" @click="exportXml">
          <Download :size="16" />
          生成模板
        </button>
      </div>
    </header>

    <aside class="sidebar">
      <section class="basic-settings">
        <button
          class="basic-settings-toggle"
          type="button"
          :aria-expanded="basicSettingsExpanded"
          aria-controls="basic-settings-content"
          @click="basicSettingsExpanded = !basicSettingsExpanded"
        >
          <div>
            <span class="eyebrow">BASIC SETTINGS</span>
            <h2>基本设置</h2>
          </div>
          <ChevronDown v-if="basicSettingsExpanded" :size="18" />
          <ChevronRight v-else :size="18" />
        </button>

        <div
          v-show="basicSettingsExpanded"
          id="basic-settings-content"
          class="basic-settings-content"
        >
          <section class="side-section">
        <div class="section-heading">
          <div>
            <span class="eyebrow">PROJECT</span>
            <h2>模板设置</h2>
          </div>
          <Settings2 :size="17" />
        </div>

        <div class="field">
          <label for="template-name">模板名称</label>
          <input
            id="template-name"
            v-model="model.name"
            type="text"
            placeholder="输入模板名称"
          />
        </div>

        <div class="field">
          <label for="template-duration">总时长 <small>ms</small></label>
          <input
            id="template-duration"
            v-model.number="model.duration"
            type="number"
            min="0"
            step="1"
          />
        </div>
          </section>

          <section class="side-section">
        <div class="section-heading compact">
          <div>
            <span class="eyebrow">TRACKS</span>
            <h2>轨道素材</h2>
          </div>
          <Layers3 :size="17" />
        </div>

        <div class="track-list">
          <div class="track-item">
            <span class="track-index">01</span>
            <div>
              <strong>固定转场层</strong>
              <small>{{
                model.tracks.overlay
                  ? fileName(model.tracks.overlay)
                  : '尚未上传'
              }}</small>
            </div>
            <div class="track-actions">
              <button
                v-if="model.tracks.overlay"
                class="plain-icon danger-hover"
                type="button"
                title="移除固定转场素材"
                @click="clearTrackFile('overlay')"
              >
                <X :size="13" />
              </button>
              <button
                class="icon-button"
                type="button"
                title="上传固定转场素材"
                @click="updateTrackFile('overlay')"
              >
                <Upload :size="14" />
              </button>
            </div>
          </div>
          <div class="track-item">
            <span class="track-index">02</span>
            <div>
              <strong>固定素材层</strong>
              <small>{{
                model.tracks.background
                  ? fileName(model.tracks.background)
                  : '尚未上传'
              }}</small>
            </div>
            <div class="track-actions">
              <button
                v-if="model.tracks.background"
                class="plain-icon danger-hover"
                type="button"
                title="移除固定素材"
                @click="clearTrackFile('background')"
              >
                <X :size="13" />
              </button>
              <button
                class="icon-button"
                type="button"
                title="上传固定素材"
                @click="updateTrackFile('background')"
              >
                <Upload :size="14" />
              </button>
            </div>
          </div>
          <div class="track-item locked">
            <span class="track-index">03</span>
            <div>
              <strong>可变素材层</strong>
              <small>默认生成 · 无需上传</small>
            </div>
            <Check :size="15" />
          </div>
          <div class="track-item">
            <span class="track-index">04</span>
            <div>
              <strong>底板音频层</strong>
              <small>{{
                model.tracks.audioBackground
                  ? fileName(model.tracks.audioBackground)
                  : '未配置时不生成'
              }}</small>
            </div>
            <div class="track-actions">
              <button
                v-if="model.tracks.audioBackground"
                class="plain-icon danger-hover"
                type="button"
                title="移除底板音频"
                @click="clearTrackFile('audioBackground')"
              >
                <X :size="13" />
              </button>
              <button
                class="icon-button"
                type="button"
                title="上传底板音频"
                @click="updateTrackFile('audioBackground')"
              >
                <Upload :size="14" />
              </button>
            </div>
          </div>
          <div class="track-item">
            <span class="track-index">05</span>
            <div>
              <strong>录音层</strong>
              <small>{{
                model.tracks.recording
                  ? fileName(model.tracks.recording)
                  : '未配置时不生成'
              }}</small>
            </div>
            <div class="track-actions">
              <button
                v-if="model.tracks.recording"
                class="plain-icon danger-hover"
                type="button"
                title="移除录音"
                @click="clearTrackFile('recording')"
              >
                <X :size="13" />
              </button>
              <button
                class="icon-button"
                type="button"
                title="上传录音"
                @click="updateTrackFile('recording')"
              >
                <Upload :size="14" />
              </button>
            </div>
          </div>
        </div>
          </section>

          <section class="side-section">
        <div class="section-heading compact">
          <div>
            <span class="eyebrow">VIDEO STYLE</span>
            <h2>视频风格</h2>
          </div>
          <WandSparkles :size="17" />
        </div>
        <div class="field">
          <label for="video-style">选择风格</label>
          <select id="video-style" v-model="model.videoStyle">
            <option
              v-for="style in videoStyleOptions"
              :key="style.value"
              :value="style.value"
            >
              {{ style.label }}
            </option>
          </select>
        </div>
        <div class="video-progress-field">
          <div class="area-inspector-row video-progress-control">
            <label class="area-inspector-label" for="video-progress"
              >LUT值</label
            >
            <input
              id="video-progress"
              v-model.number="model.progress"
              class="area-inspector-range"
              type="range"
              min="0"
              max="1"
              step="0.01"
            />
            <label class="area-inspector-value">
              <input
                v-model.number="model.progress"
                type="number"
                min="0"
                max="1"
                step="0.01"
                aria-label="视频风格进度"
                @change="model.progress = clampNumber(model.progress, 0, 1)"
              />
            </label>
          </div>
        </div>
          </section>
        </div>
      </section>

      <section class="side-section asset-library">
        <div class="section-heading asset-heading">
          <div>
            <span class="eyebrow">MEDIA ASSETS</span>
            <h2>
              素材库 <span>{{ configuredAssetCount }}</span>
            </h2>
          </div>
          <div class="heading-actions">
            <button
              class="icon-button"
              type="button"
              title="创建目录"
              @click="createGroup()"
            >
              <FolderPlus :size="16" />
            </button>
            <button
              class="icon-button"
              type="button"
              title="上传到人物镜头"
              @click="uploadToDefault"
            >
              <Upload :size="16" />
            </button>
          </div>
        </div>

        <div class="search-box">
          <Search :size="15" />
          <input
            v-model="searchKeyword"
            type="search"
            placeholder="搜索目录或素材"
          />
          <button
            v-if="searchKeyword"
            type="button"
            @click="searchKeyword = ''"
          >
            <X :size="13" />
          </button>
        </div>

        <div v-if="!model.mediaGroups.length" class="empty-library">
          <Folder :size="28" />
          <strong>素材库还是空的</strong>
          <span>创建目录，或直接上传到人物镜头</span>
          <button
            class="button button-soft"
            type="button"
            @click="uploadToDefault"
          >
            <Upload :size="15" />
            上传视频
          </button>
        </div>

        <div class="folder-list">
          <article
            v-for="group in filteredGroups"
            :key="group.id"
            class="folder-card"
            :data-group-id="group.id"
          >
            <div class="folder-top">
              <button
                class="disclosure"
                type="button"
                @click="group.expanded = !group.expanded"
              >
                <ChevronDown v-if="group.expanded" :size="16" />
                <ChevronRight v-else :size="16" />
              </button>
              <div class="folder-icon"><Folder :size="16" /></div>
              <input
                v-model="group.name"
                class="folder-name"
                aria-label="目录名称"
              />
              <span class="count-badge">{{
                visibleAssetsForGroup(group).length
              }}</span>
              <button
                class="plain-icon"
                type="button"
                :title="isAudioGroup(group) ? '上传音频' : '上传视频'"
                @click="uploadToGroup(group)"
              >
                <Plus :size="15" />
              </button>
              <button
                class="plain-icon danger-hover"
                type="button"
                title="删除目录"
                @click="removeGroup(group)"
              >
                <Trash2 :size="14" />
              </button>
            </div>

            <div v-if="group.expanded" class="folder-content">
              <div class="constraint-row">
                <label>
                  最短
                  <span
                    ><input
                      v-model.number="group.minDuration"
                      type="number"
                      min="0"
                    />
                    ms</span
                  >
                </label>
                <i></i>
                <label>
                  最长
                  <span
                    ><input
                      v-model.number="group.maxDuration"
                      type="number"
                      min="0"
                    />
                    ms</span
                  >
                </label>
              </div>

              <div
                v-if="visibleAssetsForGroup(group).length"
                class="asset-list"
              >
                <div
                  v-for="asset in visibleAssetsForGroup(group)"
                  :key="asset.id"
                  class="asset-row"
                >
                  <div class="asset-thumb">
                    <Music2
                      v-if="isAudioGroup(group)"
                      :size="16"
                      class="audio-asset-icon"
                    />
                    <img
                      v-else-if="asset.thumbnailUrl"
                      :src="asset.thumbnailUrl"
                      alt=""
                      draggable="false"
                    />
                    <LoaderCircle
                      v-else-if="asset.thumbnailStatus === 'loading'"
                      :size="16"
                      class="thumbnail-spinner"
                    />
                    <AlertTriangle
                      v-else-if="asset.thumbnailStatus === 'error'"
                      :size="16"
                    />
                    <Video v-else :size="16" />
                  </div>
                  <div class="asset-copy">
                    <strong :title="asset.sourcePath || asset.name">{{
                      asset.name
                    }}</strong>
                    <small>template/assets/</small>
                  </div>
                  <button
                    class="plain-icon danger-hover"
                    type="button"
                    title="删除素材"
                    @click="removeAsset(group, asset)"
                  >
                    <X :size="14" />
                  </button>
                </div>
              </div>
              <button
                v-else
                class="folder-drop"
                type="button"
                @click="uploadToGroup(group)"
              >
                <Upload :size="15" />
                <span>{{
                  isAudioGroup(group)
                    ? '上传音频到此目录'
                    : '上传视频到此目录'
                }}</span>
              </button>
            </div>
          </article>
        </div>
      </section>
    </aside>

    <main
      class="workspace"
      :class="{ 'drawer-active': selectedClip, empty: !model.clips.length }"
    >
      <div class="workspace-head">
        <div>
          <span class="eyebrow">CLIP SEQUENCE</span>
          <h1>片段编排</h1>
          <p>添加片段并设置每个画面区域、字幕和转场</p>
        </div>
      </div>

      <div v-if="invalidAreaCount" class="inline-alert">
        <AlertTriangle :size="16" />
        有 {{ invalidAreaCount }} 个画面区域的素材已被删除，请重新选择。
      </div>

      <section class="canvas" :class="{ empty: !model.clips.length }">
        <div v-if="!model.clips.length" class="empty-canvas-copy">
          <div class="empty-visual">
            <div class="film-line"></div>
            <Sparkles :size="18" />
          </div>
          <span class="eyebrow">START BUILDING</span>
          <h2>从第一个片段开始</h2>
          <p>创建 Clip 后，即可添加素材画面、字幕和片段转场。</p>
        </div>

        <div class="clip-grid">
          <article
            v-for="(clip, index) in model.clips"
            :key="clip.id"
            class="clip-card"
            :class="{ selected: selectedClipId === clip.id }"
            tabindex="0"
            @click="selectedClipId = clip.id"
            @keydown.enter="selectedClipId = clip.id"
          >
            <div class="clip-preview">
              <div class="preview-grid"></div>
              <div class="preview-number">
                {{ String(index + 1).padStart(2, '0') }}
              </div>
              <div v-if="clip.areas.length" class="preview-content">
                <Play :size="22" fill="currentColor" />
              </div>
              <div v-else class="preview-placeholder">
                <Image :size="25" />
                <span>未添加画面</span>
              </div>
              <div class="clip-duration">
                {{ formatSeconds(clip.duration) }}
              </div>
            </div>
            <div class="clip-card-body">
              <div class="clip-title">
                <strong>{{ clip.name }}</strong>
                <button
                  type="button"
                  title="片段设置"
                  @click.stop="selectedClipId = clip.id"
                >
                  <MoreHorizontal :size="17" />
                </button>
              </div>
              <div class="clip-time">
                <span>{{ formatMilliseconds(clip.starttime) }}</span>
                <ArrowRight :size="12" />
                <span>{{
                  formatMilliseconds(
                    Number(clip.starttime) + Number(clip.duration),
                  )
                }}</span>
              </div>
              <div class="clip-tags">
                <span
                  class="material-type"
                  :class="`is-${clipMaterialType(clip)}`"
                >
                  <Film :size="12" />
                  {{
                    clipMaterialType(clip) === 'fixed'
                      ? '固定素材'
                      : '可变素材'
                  }}
                </span>
                <span :class="{ muted: !clip.areas.length }"
                  ><Layers3 :size="12" /> {{ clip.areas.length }} Area</span
                >
                <span v-if="clip.subtitles.length"
                  ><Captions :size="12" /> {{ clip.subtitles.length }}</span
                >
                <span v-if="clip.transition.enabled" class="accent"
                  ><WandSparkles :size="12" /> 转场</span
                >
              </div>
            </div>
            <div v-if="selectedClipId === clip.id" class="selected-corner">
              <Check :size="12" />
            </div>
          </article>

          <button class="add-clip-card" type="button" @click="createClip">
            <span><Plus :size="24" /></span>
            <strong>{{ model.clips.length ? '添加片段' : '新建 Clip' }}</strong>
            <small>自动生成片段 ID</small>
          </button>
        </div>

        <div v-if="model.clips.length" class="sequence-summary">
          <div><Film :size="16" /> 序列概览</div>
          <div class="sequence-line">
            <span
              v-for="(clip, index) in model.clips"
              :key="clip.id"
              :class="{ active: selectedClipId === clip.id }"
              :style="{ flexGrow: Math.max(Number(clip.duration), 800) }"
              @click="selectedClipId = clip.id"
              >{{ index + 1 }}</span
            >
          </div>
          <div class="sequence-tracks-scroller">
            <div class="sequence-tracks" :style="sequenceTracksStyle">
              <div class="sequence-timeline-ruler-row">
                <span aria-hidden="true"></span>
                <div class="timeline-ruler sequence-timeline-ruler" aria-label="时间刻度">
                  <span
                    v-for="tick in sequenceTimelineMinorTicks"
                    :key="`sequence-minor-${tick.value}`"
                    class="timeline-ruler-tick is-minor"
                    :style="{ left: tick.left }"
                  ></span>
                  <span
                    v-for="tick in sequenceTimelineMajorTicks"
                    :key="`sequence-major-${tick.value}`"
                    class="timeline-ruler-tick is-major"
                    :style="{ left: tick.left }"
                  ></span>
                  <span
                    v-for="tick in sequenceTimelineMajorTicks"
                    :key="`sequence-label-${tick.value}`"
                    class="timeline-ruler-label"
                    :class="{
                      'is-start': tick.value === 0,
                      'is-end': tick.value === sequenceTimelineDuration,
                    }"
                    :style="{ left: tick.left }"
                  >
                    {{ formatSeconds(tick.value) }}
                  </span>
                </div>
              </div>
              <div class="sequence-track-row">
                <div class="sequence-track-label">
                  <span>01</span><strong>固定转场层</strong>
                </div>
                <div class="sequence-track-lane">
                  <div
                    class="sequence-track-clip is-transition is-full"
                    :class="{ 'is-empty': !model.tracks.overlay }"
                  >
                    {{ sequenceTrackFileLabel(model.tracks.overlay) }}
                  </div>
                </div>
              </div>
              <div class="sequence-track-row">
                <div class="sequence-track-label">
                  <span>02</span><strong>素材层</strong>
                </div>
                <div class="sequence-track-lane">
                  <template
                    v-for="(clip, index) in model.clips"
                    :key="`sequence-track-${clip.id}`"
                  >
                    <button
                      class="sequence-track-clip"
                      :class="{
                        active: selectedClipId === clip.id,
                        'is-fixed-material':
                          clipMaterialType(clip) === 'fixed',
                        'is-variable-material':
                          clipMaterialType(clip) === 'variable',
                      }"
                      :style="sequenceClipStyle(clip)"
                      type="button"
                      :title="sequenceClipTitle(clip)"
                      @click="selectedClipId = clip.id"
                    >
                      <span class="sequence-track-clip-copy">
                        {{ index + 1 }} ·
                        {{
                          clipMaterialType(clip) === 'fixed' ? '固定' : '可变'
                        }}
                        · {{ clip.name }}
                      </span>
                    </button>
                    <button
                      v-if="
                        clip.transition?.enabled &&
                        index < model.clips.length - 1
                      "
                      class="sequence-transition-bridge"
                      :class="{ active: selectedClipId === clip.id }"
                      :style="sequenceTransitionStyle(clip)"
                      type="button"
                      :title="`${clip.name} → ${model.clips[index + 1].name} · ${clipTransitionDescription(clip)}`"
                      :aria-label="`${clip.name}与${model.clips[index + 1].name}之间的转场`"
                      @click="selectedClipId = clip.id"
                    >
                      <span></span>
                    </button>
                  </template>
                </div>
              </div>
              <div class="sequence-track-row">
                <div class="sequence-track-label">
                  <span>03</span><strong>底板音频层</strong>
                </div>
                <div class="sequence-track-lane">
                  <div
                    class="sequence-track-clip is-audio is-full"
                    :class="{ 'is-empty': !model.tracks.audioBackground }"
                  >
                    {{ sequenceTrackFileLabel(model.tracks.audioBackground) }}
                  </div>
                </div>
              </div>
              <div class="sequence-track-row">
                <div class="sequence-track-label">
                  <span>04</span><strong>录音层</strong>
                </div>
                <div class="sequence-track-lane">
                  <div
                    class="sequence-track-clip is-recording is-full"
                    :class="{ 'is-empty': !model.tracks.recording }"
                  >
                    {{ sequenceTrackFileLabel(model.tracks.recording) }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>

    <Transition name="drawer">
      <aside v-if="selectedClip" class="clip-drawer">
        <div class="drawer-header">
          <div>
            <span class="eyebrow">CLIP SETTINGS</span>
            <h2>{{ selectedClip.name }}</h2>
          </div>
          <button
            class="icon-button"
            type="button"
            title="关闭设置"
            @click="selectedClipId = ''"
          >
            <X :size="17" />
          </button>
        </div>

        <div class="drawer-scroll">
          <section class="drawer-section">
            <div class="drawer-section-title with-action">
              <span class="step-number">01</span>
              <h3>基础信息</h3>
            </div>
            <div class="field">
              <label>片段名称</label>
              <input v-model="selectedClip.name" type="text" />
            </div>
            <div class="field clip-material-type-field">
              <label>素材类型</label>
              <div class="segmented clip-material-segmented">
                <button
                  type="button"
                  :class="{
                    active: clipMaterialType(selectedClip) === 'variable',
                  }"
                  @click="setClipMaterialType(selectedClip, 'variable')"
                >
                  可变素材
                </button>
                <button
                  type="button"
                  :class="{
                    active: clipMaterialType(selectedClip) === 'fixed',
                  }"
                  @click="setClipMaterialType(selectedClip, 'fixed')"
                >
                  固定素材
                </button>
              </div>
            </div>
            <div class="field-grid">
              <div class="field">
                <label>开始时间 <small>ms</small></label>
                <input
                  :value="selectedClip.starttime"
                  type="number"
                  min="0"
                  step="1"
                  @focus="captureTimelineEdit"
                  @input="updateClipTiming(selectedClip, 'starttime', $event)"
                  @blur="finishTimelineEdit"
                />
              </div>
              <div class="field">
                <label>片段时长 <small>ms</small></label>
                <input
                  :value="selectedClip.duration"
                  type="number"
                  min="1"
                  step="1"
                  @focus="captureTimelineEdit"
                  @input="updateClipTiming(selectedClip, 'duration', $event)"
                  @blur="finishTimelineEdit"
                />
              </div>
            </div>
            <p class="field-note">
              <CircleHelp :size="13" /> 连续片段的后续开始时间会实时联动
            </p>
          </section>

          <section class="drawer-section">
            <div class="drawer-section-title">
              <div>
                <span class="step-number">02</span>
                <h3>画面区域</h3>
              </div>
            </div>
            <button
              class="area-manager-entry"
              type="button"
              @click="openAreaManager"
            >
              <Maximize2 :size="20" />
              <span>
                <strong>{{
                  selectedClip.areas.length
                    ? `管理 ${selectedClip.areas.length} 个画面区域`
                    : '设置画面区域'
                }}</strong>
                <small>在弹窗中添加、查看和编辑区域</small>
              </span>
              <ChevronRight :size="16" />
            </button>
          </section>

          <section class="drawer-section">
            <div class="drawer-section-title with-toggle">
              <div>
                <span class="step-number">03</span>
                <h3>片段转场</h3>
              </div>
              <label class="switch">
                <input
                  v-model="selectedClip.transition.enabled"
                  type="checkbox"
                />
                <span></span>
              </label>
            </div>
            <div
              v-if="selectedClip.transition.enabled"
              class="transition-panel"
            >
              <div class="field">
                <label>转场特效</label>
                <select v-model="selectedClip.transition.effect">
                  <option
                    v-for="effect in transitionEffects"
                    :key="effect.value"
                    :value="effect.value"
                  >
                    {{ effect.label }} · {{ effect.value }}
                  </option>
                </select>
              </div>
              <div class="area-inspector-row transition-duration-control">
                <span class="area-inspector-label">转场时长</span>
                <input
                  v-model.number="selectedClip.transition.duration"
                  class="area-inspector-range"
                  type="range"
                  min="0"
                  max="2000"
                  step="10"
                />
                <label class="area-inspector-value">
                  <input
                    v-model.number="selectedClip.transition.duration"
                    type="number"
                    min="0"
                    max="2000"
                    step="10"
                    aria-label="转场时长"
                    @change="
                      selectedClip.transition.duration = clampNumber(
                        selectedClip.transition.duration,
                        0,
                        2000,
                      )
                    "
                  />
                  <small>ms</small>
                </label>
              </div>
            </div>
            <p v-else class="disabled-hint">
              开启后，该片段到下一个片段时应用转场。
            </p>
          </section>

          <section class="drawer-section">
            <div class="drawer-section-title with-action">
              <div>
                <span class="step-number">04</span>
                <h3>字幕</h3>
                <em>{{ selectedClip.subtitles.length }}</em>
              </div>
              <button class="mini-button" type="button" @click="createSubtitle">
                <Plus :size="14" /> 添加
              </button>
            </div>

            <div v-if="selectedClip.subtitles.length" class="subtitle-list">
              <article
                v-for="(subtitle, index) in selectedClip.subtitles"
                :key="subtitle.id"
                class="subtitle-card"
              >
                <button
                  class="subtitle-row"
                  type="button"
                  @click="editSubtitle(subtitle)"
                >
                  <span class="subtitle-row-index">
                    <Captions :size="15" />
                  </span>
                  <span class="subtitle-row-main">
                    <strong>{{
                      subtitle.defaultText || `字幕 ${index + 1}`
                    }}</strong>
                    <small>
                      {{
                        subtitle.timeMode === 'absolute'
                          ? '绝对时间'
                          : '相对时间'
                      }}
                      · {{ subtitle.time }}ms · {{ subtitle.duration }}ms
                    </small>
                  </span>
                  <ChevronRight :size="15" />
                </button>
                <button
                  class="plain-icon danger-hover"
                  type="button"
                  title="删除字幕"
                  @click.stop="removeSubtitle(subtitle)"
                >
                  <Trash2 :size="14" />
                </button>
              </article>
            </div>
            <button
              v-else
              class="empty-block compact"
              type="button"
              @click="createSubtitle"
            >
              <Captions :size="19" />
              <strong>还没有字幕</strong>
              <span>可添加多条字幕配置</span>
            </button>
          </section>
        </div>

        <div class="drawer-footer">
          <div class="drawer-move">
            <button
              type="button"
              :disabled="model.clips.indexOf(selectedClip) === 0"
              @click="moveClip(selectedClip, -1)"
            >
              前移
            </button>
            <button
              type="button"
              :disabled="
                model.clips.indexOf(selectedClip) === model.clips.length - 1
              "
              @click="moveClip(selectedClip, 1)"
            >
              后移
            </button>
          </div>
          <button
            class="button button-danger-ghost"
            type="button"
            @click="removeClip(selectedClip)"
          >
            <Trash2 :size="15" /> 删除片段
          </button>
        </div>
      </aside>
    </Transition>

    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="areaDialogOpen"
          class="modal-backdrop"
          @mousedown.self="closeAreaDialog"
        >
        <div
          class="area-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="area-dialog-title"
        >
          <div class="modal-header">
            <div>
              <span class="eyebrow">AREA SETTINGS</span>
              <h2 id="area-dialog-title">画面区域设置</h2>
              <p>所有修改都会自动保存，可连续添加多个区域</p>
            </div>
            <button
              class="icon-button"
              type="button"
              @click="closeAreaDialog"
            >
              <X :size="18" />
            </button>
          </div>

          <div class="area-track-upload area-track-upload-required">
            <div class="area-track-upload-copy">
              <span class="area-track-upload-icon"><Video :size="17" /></span>
              <div>
                <strong>顶层视频 <em>必需</em></strong>
                <span :title="model.tracks.overlay || ''">{{
                  model.tracks.overlay
                    ? fileName(model.tracks.overlay)
                    : '请上传一个透明前景视频'
                }}</span>
              </div>
            </div>
            <div class="upload-actions">
              <button
                v-if="model.tracks.overlay"
                class="plain-icon danger-hover"
                type="button"
                title="移除顶层视频"
                @click="clearTrackFile('overlay')"
              >
                <X :size="14" />
              </button>
              <button
                class="button button-secondary area-track-upload-button"
                type="button"
                @click="updateTrackFile('overlay')"
              >
                <Upload :size="14" />
                {{ model.tracks.overlay ? '重新上传' : '上传视频' }}
              </button>
            </div>
          </div>

          <div class="modal-body area-modal-body">
            <div class="area-editor-layout">
              <aside class="area-asset-pane">
                <nav class="asset-folder-tabs" aria-label="素材目录筛选">
                  <button
                    v-for="tab in areaAssetGroupTabs"
                    :key="tab.id"
                    class="asset-folder-tab"
                    :class="{ active: areaAssetGroupFilter === tab.id }"
                    type="button"
                    :title="tab.name"
                    @click="areaAssetGroupFilter = tab.id"
                  >
                    <span>{{ tab.name }}</span>
                    <small>{{ tab.count }}</small>
                  </button>
                </nav>

                <section class="asset-picker-panel">
                  <div class="modal-section-head">
                    <div>
                      <span>01</span>
                      <h3>选择视频素材</h3>
                    </div>
                    <small>{{ areaPickerAssets.length }} 个</small>
                  </div>
                  <div v-if="areaPickerAssets.length" class="asset-picker">
                    <button
                      v-for="asset in areaPickerAssets"
                      :key="asset.id"
                      type="button"
                      draggable="true"
                      :disabled="!areaDraft || isBoundGeneratedArea(areaDraft)"
                      :class="{
                        selected: areaDraft && areaDraft.assetId === asset.id,
                      }"
                      @click="selectAreaAsset(asset.id)"
                      @dragstart="startAreaAssetDrag($event, asset)"
                      @dragend="finishAreaAssetDrag"
                    >
                      <div class="picker-thumb">
                        <img
                          v-if="asset.thumbnailUrl"
                          :src="asset.thumbnailUrl"
                          alt=""
                          draggable="false"
                        />
                        <LoaderCircle
                          v-else-if="asset.thumbnailStatus === 'loading'"
                          :size="20"
                          class="thumbnail-spinner"
                        />
                        <AlertTriangle
                          v-else-if="asset.thumbnailStatus === 'error'"
                          :size="20"
                        />
                        <Video v-else :size="20" />
                        <span
                          v-if="areaDraft && areaDraft.assetId === asset.id"
                          ><Check :size="13"
                        /></span>
                      </div>
                      <strong>{{ asset.name }}</strong>
                      <small>{{ asset.groupName }}</small>
                    </button>
                  </div>
                  <div v-else class="no-assets">
                    <FolderPlus :size="22" />
                    <div>
                      <strong>{{
                        allAssets.length
                          ? '该目录下还没有视频'
                          : '素材库中还没有视频'
                      }}</strong>
                      <span>{{
                        allAssets.length
                          ? '请选择其他目录。'
                          : '请先在左侧上传素材，再回来选择。'
                      }}</span>
                    </div>
                  </div>
                </section>
              </aside>

              <div
                class="area-main-pane"
                :class="{ 'has-area-draft': areaDraft }"
              >
                <section class="area-dialog-manager">
                  <div class="area-dialog-manager-head">
                    <div>
                      <strong>画面区域</strong>
                      <small
                        >{{ selectedClip.areas.length }} 个已保存区域</small
                      >
                    </div>
                    <button class="mini-button" type="button" @click="newArea">
                      <Plus :size="14" /> 添加区域
                    </button>
                  </div>

                  <div class="area-list area-dialog-list">
                    <div
                      v-for="(area, index) in selectedClip.areas"
                      :key="area.id"
                      class="area-item"
                      :class="{
                        active: areaDraft && areaDraft.id === area.id,
                      }"
                      role="button"
                      tabindex="0"
                      @click="editArea(area)"
                      @keydown.enter="editArea(area)"
                    >
                      <span class="area-index">{{
                        String(index + 1).padStart(2, '0')
                      }}</span>
                      <span class="area-main">
                        <strong>{{ assetName(area.assetId) }}</strong>
                        <small
                          >层级 {{ area.index ?? index + 1 }} · {{ area.x }},
                          {{ area.y }} · {{ area.width }} ×
                          {{ area.height }}</small
                        >
                      </span>
                      <span
                        class="area-state"
                        :class="{ empty: !area.assetId }"
                        >{{
                          area.isMirrorGenerated
                            ? '镜像绑定'
                            : area.isTriptychGenerated
                              ? '三分屏绑定'
                            : area.assetId
                              ? '已配置'
                              : '待选择'
                        }}</span
                      >
                      <button
                        class="plain-icon area-delete"
                        type="button"
                        title="删除 Area"
                        @click.stop="removeArea(area)"
                      >
                        <Trash2 :size="13" />
                      </button>
                      <ChevronRight :size="15" />
                    </div>
                  </div>
                </section>

                <template v-if="areaDraft">
                  <div class="area-settings-stack">
                  <div class="modal-columns">
                  <section
                    class="modal-section"
                    :class="{ bound: isBoundGeneratedArea(areaDraft) }"
                  >
                    <button
                      class="modal-section-head modal-section-toggle"
                      type="button"
                      :aria-expanded="areaModalSectionsExpanded.transform"
                      @click="
                        areaModalSectionsExpanded.transform =
                          !areaModalSectionsExpanded.transform
                      "
                    >
                      <div>
                        <span>02</span>
                        <h3>画面变换</h3>
                      </div>
                      <ChevronDown
                        v-if="areaModalSectionsExpanded.transform"
                        :size="15"
                      />
                      <ChevronRight v-else :size="15" />
                    </button>
                    <div
                      v-show="areaModalSectionsExpanded.transform"
                      class="modal-section-collapse"
                    >
                        <div class="modal-section-collapse-inner">
                    <div class="area-inspector-choice-row">
                      <span>镜像方式</span>
                      <div class="segmented three">
                        <button
                          :class="{ active: areaDraft.mirror === 'none' }"
                          type="button"
                          :disabled="
                            isBoundGeneratedArea(areaDraft) ||
                            isLayoutSourceArea(areaDraft)
                          "
                          @click="areaDraft.mirror = 'none'"
                        >
                          无
                        </button>
                        <button
                          :class="{ active: areaDraft.mirror === 'horizontal' }"
                          type="button"
                          :disabled="
                            isBoundGeneratedArea(areaDraft) ||
                            isLayoutSourceArea(areaDraft)
                          "
                          @click="areaDraft.mirror = 'horizontal'"
                        >
                          水平
                        </button>
                        <button
                          :class="{ active: areaDraft.mirror === 'vertical' }"
                          type="button"
                          :disabled="
                            isBoundGeneratedArea(areaDraft) ||
                            isLayoutSourceArea(areaDraft)
                          "
                          @click="areaDraft.mirror = 'vertical'"
                        >
                          垂直
                        </button>
                      </div>
                    </div>
                    <div class="area-inspector-controls">
                      <div class="area-inspector-row">
                        <span class="area-inspector-label">播放速度</span>
                        <input
                          v-model.number="areaDraft.speed"
                          class="area-inspector-range"
                          type="range"
                          :disabled="isBoundGeneratedArea(areaDraft)"
                          min="0"
                          max="1"
                          step="0.01"
                        />
                        <label class="area-inspector-value">
                        <input
                          v-model.number="areaDraft.speed"
                          type="number"
                          :disabled="isBoundGeneratedArea(areaDraft)"
                          min="0"
                          max="1"
                          step="0.01"
                        />
                          <small>x</small>
                        </label>
                      </div>
                      <div class="area-inspector-row">
                        <span class="area-inspector-label">旋转角度</span>
                        <span class="area-inspector-spacer"></span>
                        <label class="area-inspector-value">
                        <input
                          v-model.number="areaDraft.rotate"
                          type="number"
                          :disabled="isBoundGeneratedArea(areaDraft)"
                          min="-360"
                          max="360"
                        />
                          <small>°</small>
                        </label>
                      </div>
                      <div class="area-inspector-row">
                        <span class="area-inspector-label">透明度</span>
                      <input
                        v-model.number="areaDraft.opacity"
                        class="area-inspector-range"
                        type="range"
                        :disabled="isBoundGeneratedArea(areaDraft)"
                        min="0"
                        max="1"
                        step="0.01"
                        @change="normalizeAreaOpacity()"
                      />
                        <label class="area-inspector-value">
                          <input
                            v-model.number="areaDraft.opacity"
                            type="number"
                            :disabled="isBoundGeneratedArea(areaDraft)"
                            min="0"
                            max="1"
                            step="0.01"
                            @change="normalizeAreaOpacity()"
                          />
                        </label>
                      </div>
                    </div>
                        </div>
                    </div>
                  </section>

                  <section
                    class="modal-section"
                    :class="{ bound: isBoundGeneratedArea(areaDraft) }"
                  >
                    <button
                      class="modal-section-head modal-section-toggle"
                      type="button"
                      :aria-expanded="areaModalSectionsExpanded.position"
                      @click="
                        areaModalSectionsExpanded.position =
                          !areaModalSectionsExpanded.position
                      "
                    >
                      <div>
                        <span>03</span>
                        <h3>位置与尺寸</h3>
                      </div>
                      <span class="modal-section-toggle-meta">
                        <small>{{
                          isBoundGeneratedArea(areaDraft)
                            ? '跟随原区域'
                            : isQuadSourceArea(areaDraft)
                              ? '四分屏联动'
                            : isTriptychSourceArea(areaDraft)
                              ? '三分屏联动'
                              : '固定 16:9'
                        }}</small>
                        <ChevronDown
                          v-if="areaModalSectionsExpanded.position"
                          :size="15"
                        />
                        <ChevronRight v-else :size="15" />
                      </span>
                    </button>
                    <div
                      v-show="areaModalSectionsExpanded.position"
                      class="modal-section-collapse"
                    >
                        <div class="modal-section-collapse-inner">
                    <div class="area-inspector-controls">
                      <div class="area-inspector-row area-inspector-pair-row">
                        <span class="area-inspector-label">位置</span>
                        <div class="area-inspector-pair">
                          <label class="area-inspector-value has-prefix">
                            <small>X</small>
                            <input
                              v-model.number="areaDraft.x"
                              type="number"
                              :disabled="isBoundGeneratedArea(areaDraft)"
                              min="0"
                              max="1919"
                              @change="normalizeAreaDraft"
                            />
                          </label>
                          <label class="area-inspector-value has-prefix">
                            <small>Y</small>
                            <input
                              v-model.number="areaDraft.y"
                              type="number"
                              :disabled="isBoundGeneratedArea(areaDraft)"
                              min="0"
                              max="1079"
                              @change="normalizeAreaDraft"
                            />
                          </label>
                        </div>
                      </div>
                      <div class="area-inspector-row area-inspector-pair-row">
                        <span class="area-inspector-label">尺寸</span>
                        <div class="area-inspector-pair">
                          <label class="area-inspector-value has-prefix">
                            <small>W</small>
                            <input
                              v-model.number="areaDraft.width"
                              type="number"
                              :disabled="isBoundGeneratedArea(areaDraft)"
                              min="16"
                              max="1920"
                              step="16"
                              @change="normalizeAreaDraft('width')"
                            />
                          </label>
                          <label class="area-inspector-value has-prefix">
                            <small>H</small>
                            <input
                              v-model.number="areaDraft.height"
                              type="number"
                              :disabled="isBoundGeneratedArea(areaDraft)"
                              min="9"
                              max="1080"
                              step="9"
                              @change="normalizeAreaDraft('height')"
                            />
                          </label>
                        </div>
                      </div>
                      <div class="area-inspector-row">
                        <span class="area-inspector-label">层级</span>
                        <span class="area-inspector-spacer"></span>
                        <label class="area-inspector-value">
                          <input
                            v-model.number="areaDraft.index"
                            type="number"
                            min="0"
                            step="1"
                            @change="normalizeAreaIndex"
                          />
                        </label>
                      </div>
                    </div>
                        </div>
                    </div>
                  </section>

                  <section
                    class="modal-section beauty-modal-section"
                    :class="{ bound: isBoundGeneratedArea(areaDraft) }"
                  >
                    <button
                      class="modal-section-head modal-section-toggle"
                      type="button"
                      :aria-expanded="areaModalSectionsExpanded.beauty"
                      @click="
                        areaModalSectionsExpanded.beauty =
                          !areaModalSectionsExpanded.beauty
                      "
                    >
                      <div>
                        <span>04</span>
                        <h3>美颜</h3>
                      </div>
                      <ChevronDown
                        v-if="areaModalSectionsExpanded.beauty"
                        :size="15"
                      />
                      <ChevronRight v-else :size="15" />
                    </button>
                    <div
                      v-show="areaModalSectionsExpanded.beauty"
                      class="modal-section-collapse"
                    >
                      <div class="modal-section-collapse-inner">
                        <fieldset
                          class="area-beauty-settings"
                          :disabled="isBoundGeneratedArea(areaDraft)"
                        >
                          <div class="area-beauty-toolbar">
                            <span>针对当前画面区域调整美颜效果</span>
                            <button
                              type="button"
                              class="area-beauty-reset"
                              @click="resetAreaBeauty"
                            >
                              恢复默认
                            </button>
                          </div>

                          <div class="area-beauty-grid">
                            <div class="area-beauty-control is-select">
                              <span class="area-inspector-label">LUT 风格</span>
                              <select v-model="areaDraft.beauty.lutStyle">
                                <option value="none">无</option>
                                <option value="clear">清透</option>
                                <option value="warm">暖阳</option>
                                <option value="cinematic">电影</option>
                                <option value="vintage">复古</option>
                                <option value="cool">冷调</option>
                              </select>
                            </div>

                            <div class="area-beauty-control">
                              <span class="area-inspector-label">LUT 强度</span>
                              <input
                                v-model.number="areaDraft.beauty.lutIntensity"
                                class="area-inspector-range"
                                type="range"
                                min="0"
                                max="100"
                                step="1"
                                @change="
                                  normalizeAreaBeautyValue('lutIntensity')
                                "
                              />
                              <label class="area-inspector-value">
                                <input
                                  v-model.number="areaDraft.beauty.lutIntensity"
                                  type="number"
                                  min="0"
                                  max="100"
                                  step="1"
                                  @change="normalizeAreaBeautyValue('lutIntensity')"
                                />
                                <small>%</small>
                              </label>
                            </div>

                            <div class="area-beauty-control">
                              <span class="area-inspector-label">磨皮</span>
                              <input
                                v-model.number="areaDraft.beauty.smoothing"
                                class="area-inspector-range"
                                type="range"
                                min="0"
                                max="100"
                                step="1"
                                @change="normalizeAreaBeautyValue('smoothing')"
                              />
                              <label class="area-inspector-value">
                                <input
                                  v-model.number="areaDraft.beauty.smoothing"
                                  type="number"
                                  min="0"
                                  max="100"
                                  step="1"
                                  @change="normalizeAreaBeautyValue('smoothing')"
                                />
                                <small>%</small>
                              </label>
                            </div>

                            <div class="area-beauty-control">
                              <span class="area-inspector-label">美白</span>
                              <input
                                v-model.number="areaDraft.beauty.whitening"
                                class="area-inspector-range"
                                type="range"
                                min="0"
                                max="100"
                                step="1"
                                @change="normalizeAreaBeautyValue('whitening')"
                              />
                              <label class="area-inspector-value">
                                <input
                                  v-model.number="areaDraft.beauty.whitening"
                                  type="number"
                                  min="0"
                                  max="100"
                                  step="1"
                                  @change="normalizeAreaBeautyValue('whitening')"
                                />
                                <small>%</small>
                              </label>
                            </div>

                            <div class="area-beauty-skin">
                              <div class="area-beauty-skin-head">
                                <span>肤色</span>
                                <div
                                  class="area-skin-tone-options"
                                  role="group"
                                  aria-label="肤色选择"
                                >
                                  <button
                                    v-for="option in AREA_SKIN_TONE_OPTIONS"
                                    :key="option.value"
                                    type="button"
                                    class="area-skin-tone-option"
                                    :class="{
                                      active:
                                        areaDraft.beauty.skinTone ===
                                        option.value,
                                      off: option.value === 'off',
                                    }"
                                    :style="{ '--swatch-color': option.color }"
                                    :title="option.label"
                                    :aria-label="option.label"
                                    :aria-pressed="
                                      areaDraft.beauty.skinTone === option.value
                                    "
                                    @click="
                                      areaDraft.beauty.skinTone = option.value
                                    "
                                  ></button>
                                </div>
                              </div>
                              <div
                                v-if="areaDraft.beauty.skinTone !== 'off'"
                                class="area-skin-adjustments"
                              >
                                <div class="area-beauty-control">
                                  <span class="area-inspector-label">程度</span>
                                  <input
                                    v-model.number="
                                      areaDraft.beauty.skinIntensity
                                    "
                                    class="area-inspector-range"
                                    type="range"
                                    min="0"
                                    max="100"
                                    step="1"
                                    @change="
                                      normalizeAreaBeautyValue('skinIntensity')
                                    "
                                  />
                                  <label class="area-inspector-value">
                                    <input
                                      v-model.number="areaDraft.beauty.skinIntensity"
                                      type="number"
                                      min="0"
                                      max="100"
                                      step="1"
                                      @change="normalizeAreaBeautyValue('skinIntensity')"
                                    />
                                    <small>%</small>
                                  </label>
                                </div>
                              </div>
                            </div>

                            <div class="area-beauty-switches">
                              <label>
                                <span>视频去抖动</span>
                                <span class="switch">
                                  <input
                                    v-model="areaDraft.beauty.stabilization"
                                    type="checkbox"
                                  />
                                  <span></span>
                                </span>
                              </label>
                              <label>
                                <span>一键美颜</span>
                                <span class="switch">
                                  <input
                                    v-model="areaDraft.beauty.oneClickBeauty"
                                    type="checkbox"
                                  />
                                  <span></span>
                                </span>
                              </label>
                            </div>
                          </div>
                        </fieldset>
                      </div>
                    </div>
                  </section>
                  </div>

                  <section class="canvas-preview-section">
                  <div
                    class="screen-preview"
                    :class="{ 'asset-drag-over': areaCanvasDragOver }"
                    @dragover="handleAreaCanvasDragOver"
                    @dragleave="handleAreaCanvasDragLeave"
                    @drop="dropAssetOnAreaCanvas"
                  >
                    <div class="screen-safe"></div>
                    <div
                      v-for="preview in otherAreaPreviews"
                      :key="preview.id"
                      class="area-rect area-rect-readonly"
                      :class="{ triptych: preview.isTriptych }"
                      :style="preview.style"
                      :title="`${preview.label} · 点击选择`"
                      @click.stop="selectAreaFromCanvas(preview.id)"
                      @contextmenu.prevent.stop="
                        openAreaContextMenu($event, preview.id)
                      "
                    >
                      <img
                        v-if="preview.thumbnailUrl"
                        class="area-preview-image"
                        :src="preview.thumbnailUrl"
                        :style="preview.imageStyle"
                        alt=""
                        draggable="false"
                      />
                      <span class="area-size-label">{{ preview.label }}</span>
                    </div>
                    <div
                      class="area-rect area-rect-active"
                      :class="{
                        bound: isBoundGeneratedArea(areaDraft),
                        triptych: areaDraft.triptychGroupId,
                      }"
                      :style="areaPreviewStyle"
                      :title="
                        isBoundGeneratedArea(areaDraft)
                          ? '绑定区域跟随原区域，位置和尺寸不可修改'
                          : '拖动调整位置'
                      "
                      @pointerdown="startAreaInteraction($event, 'move')"
                      @contextmenu.prevent.stop="
                        openAreaContextMenu($event, areaDraft.id)
                      "
                    >
                      <img
                        v-if="areaPreviewThumbnailUrl"
                        class="area-preview-image"
                        :src="areaPreviewThumbnailUrl"
                        :style="areaPreviewImageStyle"
                        alt=""
                        draggable="false"
                      />
                      <span class="area-size-label"
                        >{{ areaDraft.width }} × {{ areaDraft.height }}</span
                      >
                      <template v-if="!isBoundGeneratedArea(areaDraft)">
                      <button
                        class="area-resize-handle handle-nw"
                        type="button"
                        aria-label="从左上角调整区域大小"
                        @pointerdown.stop="startAreaInteraction($event, 'nw')"
                      ></button>
                      <button
                        class="area-resize-handle handle-ne"
                        type="button"
                        aria-label="从右上角调整区域大小"
                        @pointerdown.stop="startAreaInteraction($event, 'ne')"
                      ></button>
                      <button
                        class="area-resize-handle handle-sw"
                        type="button"
                        aria-label="从左下角调整区域大小"
                        @pointerdown.stop="startAreaInteraction($event, 'sw')"
                      ></button>
                      <button
                        class="area-resize-handle handle-se"
                        type="button"
                        aria-label="从右下角调整区域大小"
                        @pointerdown.stop="startAreaInteraction($event, 'se')"
                      ></button>
                      </template>
                    </div>
                  </div>
                  </section>
                  </div>
                </template>
                <div v-else class="area-editor-empty">
                  <Maximize2 :size="24" />
                  <strong>还没有画面区域</strong>
                  <span>点击上方“添加区域”后，再选择或拖入视频素材</span>
                </div>
              </div>
            </div>
          </div>

          <div class="area-track-upload area-track-upload-optional">
            <div class="area-track-upload-copy">
              <span class="area-track-upload-icon"><Film :size="17" /></span>
              <div>
                <strong>背景底层 <em class="optional">可选</em></strong>
                <span :title="model.tracks.background || ''">{{
                  model.tracks.background
                    ? fileName(model.tracks.background)
                    : '未上传时不生成底层背景'
                }}</span>
              </div>
            </div>
            <div class="upload-actions">
              <button
                v-if="model.tracks.background"
                class="plain-icon danger-hover"
                type="button"
                title="移除背景视频"
                @click="clearTrackFile('background')"
              >
                <X :size="14" />
              </button>
              <button
                class="button button-secondary area-track-upload-button"
                type="button"
                @click="updateTrackFile('background')"
              >
                <Upload :size="14" />
                {{ model.tracks.background ? '重新上传' : '上传视频' }}
              </button>
            </div>
          </div>

          <div class="modal-footer">
            <span class="auto-save-note">{{
              areaDraft ? '修改内容已自动保存' : '请先添加画面区域'
            }}</span>
            <button
              class="button button-primary"
              type="button"
              @click="closeAreaDialog"
            >
              <Check :size="16" /> 完成
            </button>
          </div>

          <div
            v-if="areaContextMenu.open"
            class="area-context-menu"
            :style="{
              left: `${areaContextMenu.x}px`,
              top: `${areaContextMenu.y}px`,
            }"
            @pointerdown.stop
            @contextmenu.prevent
          >
            <div class="area-context-item has-submenu">
              <span>镜像</span>
              <ChevronRight :size="14" />
              <div class="area-context-submenu">
                <button
                  v-for="direction in mirrorDirections"
                  :key="direction.value"
                  type="button"
                  :disabled="
                    !canMirrorArea(areaContextMenu.areaId, direction.value)
                  "
                  @click.stop="createMirroredArea(direction.value)"
                >
                  <span>{{ direction.label }}</span>
                  <Check
                    v-if="
                      areaMirrorDirectionCreated(
                        areaContextMenu.areaId,
                        direction.value,
                      )
                    "
                    :size="12"
                  />
                </button>
              </div>
            </div>
            <div class="area-context-item has-submenu">
              <span>三分屏</span>
              <ChevronRight :size="14" />
              <div class="area-context-submenu">
                <button
                  v-for="rotation in triptychRotations"
                  :key="rotation.value"
                  type="button"
                  :disabled="!canCreateTriptych(areaContextMenu.areaId)"
                  @click.stop="createTriptych(rotation.value)"
                >
                  <span>{{ rotation.label }}</span>
                </button>
              </div>
            </div>
            <button
              class="area-context-item area-context-action"
              type="button"
              :disabled="!canCreateQuad(areaContextMenu.areaId)"
              @click.stop="createQuad"
            >
              <span>四分屏</span>
            </button>
          </div>
        </div>
        </div>
      </Transition>
    </Teleport>

    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="subtitleDialogOpen && subtitleDraft"
          class="modal-backdrop"
          @mousedown.self="subtitleDialogOpen = false"
        >
        <div
          class="area-modal subtitle-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="subtitle-dialog-title"
        >
          <div class="modal-header">
            <div>
              <span class="eyebrow">SUBTITLE SETTINGS</span>
              <h2 id="subtitle-dialog-title">
                {{ subtitleIsNew ? '添加字幕' : '编辑字幕' }}
              </h2>
              <p>设置字幕文案、时间、字体样式和画布位置</p>
            </div>
            <button
              class="icon-button"
              type="button"
              title="关闭字幕设置"
              @click="subtitleDialogOpen = false"
            >
              <X :size="18" />
            </button>
          </div>

          <div class="modal-body subtitle-modal-body">
            <div class="subtitle-settings-grid">
              <section class="modal-section">
                <div class="modal-section-head">
                  <div>
                    <span>01</span>
                    <h3>文案与时间</h3>
                  </div>
                </div>
                <div class="field">
                  <label>默认文案</label>
                  <textarea
                    v-model="subtitleDraft.defaultText"
                    rows="3"
                    placeholder="输入需要预览和导出的字幕文字"
                  ></textarea>
                </div>
                <div class="segmented">
                  <button
                    :class="{ active: subtitleDraft.timeMode === 'relative' }"
                    type="button"
                    @click="subtitleDraft.timeMode = 'relative'"
                  >
                    相对时间
                  </button>
                  <button
                    :class="{ active: subtitleDraft.timeMode === 'absolute' }"
                    type="button"
                    @click="subtitleDraft.timeMode = 'absolute'"
                  >
                    绝对时间
                  </button>
                </div>
                <div class="field-grid">
                  <div class="field">
                    <label>开始时间 <small>ms</small></label>
                    <input
                      v-model.number="subtitleDraft.time"
                      type="number"
                      min="0"
                      step="1"
                    />
                  </div>
                  <div class="field">
                    <label>持续时间 <small>ms</small></label>
                    <input
                      v-model.number="subtitleDraft.duration"
                      type="number"
                      min="1"
                      step="1"
                    />
                  </div>
                </div>
                <div class="field-grid">
                  <div class="field">
                    <label>最少字数</label>
                    <input
                      v-model.number="subtitleDraft.minlen"
                      type="number"
                      min="0"
                    />
                  </div>
                  <div class="field">
                    <label>最多字数</label>
                    <input
                      v-model.number="subtitleDraft.maxlen"
                      type="number"
                      min="0"
                    />
                  </div>
                </div>
              </section>

              <section class="modal-section">
                <div class="modal-section-head">
                  <div>
                    <span>02</span>
                    <h3>字体与位置</h3>
                  </div>
                </div>
                <div class="field">
                  <label>字体</label>
                  <select v-model="subtitleDraft.fontFamily">
                    <option
                      v-for="font in fontOptions"
                      :key="font"
                      :value="font"
                    >
                      {{ font }}
                    </option>
                  </select>
                </div>
                <div class="field-grid font-grid">
                  <div class="field">
                    <label>字号</label>
                    <input
                      v-model.number="subtitleDraft.fontSize"
                      type="number"
                      min="1"
                    />
                  </div>
                  <div class="field">
                    <label>颜色</label>
                    <div class="color-input">
                      <input v-model="subtitleDraft.color" type="color" />
                      <input v-model="subtitleDraft.color" type="text" />
                    </div>
                  </div>
                </div>
                <div class="field-grid">
                  <div class="field">
                    <label>位置 X</label>
                    <input
                      v-model.number="subtitleDraft.x"
                      type="number"
                      min="0"
                      max="1920"
                      @change="normalizeSubtitleDraftPosition"
                    />
                  </div>
                  <div class="field">
                    <label>位置 Y</label>
                    <input
                      v-model.number="subtitleDraft.y"
                      type="number"
                      min="0"
                      max="1080"
                      @change="normalizeSubtitleDraftPosition"
                    />
                  </div>
                </div>
                <p class="field-note">
                  <CircleHelp :size="13" /> 也可以直接拖动画布中的字幕
                </p>
              </section>
            </div>

            <section class="canvas-preview-section subtitle-preview-section">
              <div class="canvas-preview-copy">
                <strong>目标画布预览</strong>
                <span>实时展示字幕文字、字体、字号、颜色和位置</span>
              </div>
              <div class="screen-preview subtitle-screen-preview">
                <div class="screen-safe"></div>
                <div
                  class="subtitle-preview-text"
                  :class="{
                    placeholder: !(subtitleDraft.defaultText || '').trim(),
                  }"
                  :style="subtitlePreviewStyle"
                  title="拖动调整字幕位置"
                  @pointerdown="startSubtitleInteraction"
                >
                  {{ subtitlePreviewText }}
                </div>
              </div>
            </section>
          </div>

          <div class="modal-footer">
            <button
              class="button button-secondary"
              type="button"
              @click="subtitleDialogOpen = false"
            >
              取消
            </button>
            <button
              class="button button-primary"
              type="button"
              @click="saveSubtitle"
            >
              <Check :size="16" /> 保存字幕
            </button>
          </div>
        </div>
        </div>
      </Transition>
    </Teleport>

    <Teleport to="body">
      <Transition name="modal">
        <div v-if="confirmDialog.open" class="modal-backdrop confirm-backdrop">
          <div class="confirm-modal" role="alertdialog" aria-modal="true">
            <div class="confirm-icon"><AlertTriangle :size="22" /></div>
            <h3>{{ confirmDialog.title }}</h3>
            <p>{{ confirmDialog.message }}</p>
            <div>
              <button
                class="button button-secondary"
                type="button"
                @click="confirmDialog.open = false"
              >
                取消
              </button>
              <button
                class="button button-danger"
                type="button"
                @click="confirmAction"
              >
                {{ confirmDialog.confirmText }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <Teleport to="body">
      <Transition name="toast">
        <div v-if="toast.visible" class="toast" :class="toast.type">
          <Check v-if="toast.type === 'success'" :size="16" />
          <AlertTriangle v-else :size="16" />
          <span>{{ toast.message }}</span>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.app-shell {
  color: #20201e;
  background: #f4f4f1;
  font-family: 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  --ink: #20201e;
  --muted: #777872;
  --subtle: #a2a39d;
  --line: #e3e3de;
  --line-strong: #d5d5cf;
  --paper: #ffffff;
  --surface: #f8f8f5;
  --canvas: #efefeb;
  --accent: #f26645;
  --accent-dark: #dc4d2c;
  --accent-soft: #fff0eb;
  --yellow: #f0bb4c;
  --green: #319b68;
  --red: #d64a43;
  --shadow-sm: 0 4px 16px rgba(32, 32, 30, 0.06);
  --shadow-lg: 0 22px 70px rgba(32, 32, 30, 0.18);
}

* {
  box-sizing: border-box;
}

.app-shell {
  position: fixed;
  inset: 64px 0 0;
  width: 100%;
  min-width: 320px;
  height: auto;
  margin: 0;
  overflow: hidden;
}

button,
input,
select,
textarea {
  font: inherit;
}

button,
label {
  -webkit-tap-highlight-color: transparent;
}

button {
  color: inherit;
}

button:focus-visible,
input:focus-visible,
select:focus-visible,
textarea:focus-visible,
label:focus-within {
  outline: 2px solid rgba(242, 102, 69, 0.34);
  outline-offset: 2px;
}

.sr-only {
  position: absolute !important;
  width: 1px !important;
  height: 1px !important;
  padding: 0 !important;
  margin: -1px !important;
  overflow: hidden !important;
  clip: rect(0, 0, 0, 0) !important;
  white-space: nowrap !important;
  border: 0 !important;
}

.app-shell {
  min-height: 0;
  background: var(--canvas);
}

.topbar {
  position: absolute;
  z-index: 50;
  inset: 0 0 auto 0;
  height: 68px;
  display: grid;
  grid-template-columns: 350px 1fr auto;
  align-items: center;
  background: rgba(255, 255, 255, 0.96);
  border-bottom: 1px solid var(--line);
}

.brand {
  height: 100%;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 20px;
  border-right: 1px solid var(--line);
}

.brand-mark {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  color: white;
  background: var(--ink);
  transform: rotate(-2deg);
}

.brand > div:last-child {
  display: flex;
  flex-direction: column;
  line-height: 1.15;
}

.brand strong {
  font-size: 16px;
  letter-spacing: 0.08em;
}

.brand span {
  margin-top: 4px;
  color: var(--muted);
  font-size: 11px;
}

.topbar-center {
  justify-self: center;
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--muted);
  font-size: 12px;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--green);
  box-shadow: 0 0 0 3px rgba(49, 155, 104, 0.12);
}

.separator {
  width: 1px;
  height: 12px;
  margin: 0 3px;
  background: var(--line-strong);
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 9px;
  padding-right: 18px;
}

.import-xml-action {
  display: none !important;
}

.button {
  min-height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 14px;
  border: 1px solid transparent;
  border-radius: 9px;
  cursor: pointer;
  font-weight: 600;
  font-size: 12px;
  white-space: nowrap;
  transition: 160ms ease;
}

.button:hover {
  transform: translateY(-1px);
}

.button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
  transform: none;
}

.preview-button:disabled {
  opacity: 1;
  color: white;
  border-color: var(--ink);
  background: var(--ink);
}

.button-primary {
  color: white;
  border-color: var(--ink);
  background: var(--ink);
  box-shadow: 0 4px 10px rgba(32, 32, 30, 0.14);
}

.button-primary:hover {
  background: #343431;
}

.button-secondary {
  color: #454641;
  border-color: var(--line-strong);
  background: var(--paper);
}

.button-secondary:hover {
  border-color: #b8b8b2;
  background: var(--surface);
}

.button-soft {
  color: var(--accent-dark);
  border-color: #f4c7ba;
  background: var(--accent-soft);
}

.button-danger {
  color: white;
  background: var(--red);
}

.button-danger-ghost {
  color: var(--red);
  border-color: #efcbc7;
  background: #fff8f7;
}

.sidebar {
  position: absolute;
  z-index: 20;
  top: 68px;
  bottom: 0;
  left: 0;
  width: 350px;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--paper);
  border-right: 1px solid var(--line);
  scrollbar-width: thin;
  scrollbar-color: #d2d2cc transparent;
}

.side-section {
  padding: 20px;
  border-bottom: 1px solid var(--line);
}

.basic-settings {
  border-bottom: 1px solid var(--line);
}

.basic-settings-toggle {
  width: 100%;
  min-height: 68px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  color: var(--ink);
  text-align: left;
  border: 0;
  background: var(--surface);
  cursor: pointer;
}

.basic-settings-toggle:hover {
  background: #f1f1ed;
}

.basic-settings-toggle h2 {
  margin: 0;
  font-size: 15px;
}

.basic-settings-toggle > svg {
  flex: 0 0 auto;
  color: var(--muted);
}

.basic-settings-content > .side-section:last-child {
  border-bottom: 0;
}

.side-section.asset-library {
  min-height: 300px;
  padding-bottom: 50px;
}

.section-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 17px;
}

.section-heading.compact {
  margin-bottom: 13px;
}

.section-heading > svg {
  color: var(--subtle);
}

.eyebrow {
  display: block;
  margin-bottom: 4px;
  color: var(--subtle);
  font-size: 9px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: 0.15em;
}

.section-heading h2,
.workspace-head h1,
.drawer-header h2,
.modal-header h2 {
  margin: 0;
  font-weight: 700;
}

.section-heading h2 {
  font-size: 15px;
}

.field {
  min-width: 0;
  margin-bottom: 13px;
}

.field:last-child {
  margin-bottom: 0;
}

.field label,
.upload-field label:not(.icon-button) {
  display: block;
  margin-bottom: 6px;
  color: #5e5f59;
  font-size: 11px;
  font-weight: 600;
}

.field label small {
  color: var(--subtle);
  font-size: 9px;
  font-weight: 500;
}

.field input,
.field select,
.field textarea,
.folder-name {
  width: 100%;
  color: var(--ink);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: var(--paper);
  transition: 150ms ease;
}

.field input,
.field select,
.folder-name {
  height: 36px;
  padding: 0 10px;
}

.field textarea {
  min-height: 58px;
  resize: vertical;
  padding: 9px 10px;
}

.field input:hover,
.field select:hover,
.field textarea:hover {
  border-color: #bfc0ba;
}

.field input:focus,
.field select:focus,
.field textarea:focus {
  border-color: var(--accent);
  outline: none;
  box-shadow: 0 0 0 3px rgba(242, 102, 69, 0.09);
}

.field-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.video-progress-field {
  margin-top: 12px;
}

.upload-field {
  min-height: 45px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 8px 8px 11px;
  border: 1px dashed var(--line-strong);
  border-radius: 9px;
  background: var(--surface);
}

.upload-field > div {
  min-width: 0;
}

.upload-actions,
.track-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 3px;
}

.upload-field label:not(.icon-button) {
  margin: 0 0 3px;
}

.upload-field span {
  display: block;
  overflow: hidden;
  color: var(--muted);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-button,
.plain-icon {
  display: inline-grid;
  flex: 0 0 auto;
  place-items: center;
  padding: 0;
  cursor: pointer;
}

.icon-button {
  width: 31px;
  height: 31px;
  color: #60615d;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--paper);
}

.icon-button:hover {
  color: var(--ink);
  border-color: #c5c5bf;
  background: var(--surface);
}

.plain-icon {
  width: 26px;
  height: 26px;
  color: var(--muted);
  border: 0;
  border-radius: 6px;
  background: transparent;
}

.plain-icon:hover {
  color: var(--ink);
  background: #ecece8;
}

.danger-hover:hover {
  color: var(--red);
  background: #fff0ee;
}

.track-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.track-item {
  min-height: 48px;
  display: grid;
  grid-template-columns: 28px 1fr auto;
  align-items: center;
  gap: 8px;
  padding: 6px 7px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--surface);
}

.track-item.locked {
  background: #fbfbf9;
}

.track-item > svg {
  margin-right: 8px;
  color: var(--green);
}

.track-index {
  width: 27px;
  height: 27px;
  display: grid;
  place-items: center;
  color: var(--subtle);
  border-radius: 6px;
  background: #ebebe6;
  font-size: 9px;
  font-weight: 700;
}

.track-item > div:not(.track-actions) {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.track-item strong {
  font-size: 11px;
  line-height: 1.35;
}

.track-item strong em {
  margin-left: 4px;
  color: var(--accent-dark);
  font-size: 8px;
  font-style: normal;
}

.track-item strong em.optional {
  color: var(--muted);
}

.track-item small {
  overflow: hidden;
  margin-top: 2px;
  color: var(--subtle);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-item .icon-button {
  width: 28px;
  height: 28px;
}

.asset-heading h2 span {
  display: inline-grid;
  min-width: 18px;
  height: 18px;
  margin-left: 4px;
  place-items: center;
  color: var(--muted);
  border-radius: 5px;
  background: #eeeeea;
  font-size: 9px;
}

.heading-actions {
  display: flex;
  gap: 5px;
}

.search-box {
  height: 34px;
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 7px;
  margin-bottom: 12px;
  padding: 0 9px;
  color: var(--subtle);
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--surface);
}

.search-box:focus-within {
  border-color: var(--line);
  background: var(--surface);
}

.search-box input {
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  font-size: 11px;
}

.search-box input:focus,
.search-box input:focus-visible {
  outline: none !important;
  box-shadow: none !important;
}

.search-box input::-webkit-search-cancel-button,
.search-box input::-webkit-search-decoration {
  display: none;
  -webkit-appearance: none;
}

.search-box input::-ms-clear {
  display: none;
}

.search-box button {
  display: grid;
  padding: 2px;
  place-items: center;
  color: var(--muted);
  border: 0;
  background: transparent;
  cursor: pointer;
}

.empty-library {
  min-height: 170px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 22px 12px;
  color: var(--subtle);
  border: 1px dashed var(--line-strong);
  border-radius: 11px;
  background: var(--surface);
  text-align: center;
}

.empty-library > svg {
  margin-bottom: 10px;
  color: #b7b8b2;
}

.empty-library strong {
  color: #62635e;
  font-size: 12px;
}

.empty-library span {
  margin: 4px 0 13px;
  font-size: 9px;
}

.empty-library .button span {
  margin: 0;
  font-size: 11px;
}

.folder-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.folder-card {
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper);
}

.folder-top {
  min-height: 42px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 6px 5px 4px;
}

.disclosure {
  width: 24px;
  height: 28px;
  display: grid;
  flex: 0 0 auto;
  padding: 0;
  place-items: center;
  color: var(--muted);
  border: 0;
  background: transparent;
  cursor: pointer;
}

.folder-icon {
  width: 27px;
  height: 27px;
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  color: #7d6641;
  border-radius: 7px;
  background: #f7e9c8;
}

.folder-name {
  min-width: 0;
  height: 28px;
  padding: 0 4px;
  border-color: transparent;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 600;
}

.folder-name:hover,
.folder-name:focus {
  border-color: var(--line-strong);
  outline: none;
  background: var(--surface);
}

.count-badge {
  min-width: 19px;
  height: 18px;
  display: grid;
  place-items: center;
  color: var(--muted);
  border-radius: 5px;
  background: #eeeeea;
  font-size: 9px;
  font-weight: 600;
}

.folder-content {
  padding: 0 9px 9px 37px;
}

.constraint-row {
  display: grid;
  grid-template-columns: 1fr 12px 1fr;
  align-items: end;
  margin-bottom: 7px;
  padding: 7px 8px;
  border-radius: 7px;
  background: var(--surface);
}

.constraint-row > i {
  width: 7px;
  height: 1px;
  margin: 0 auto 8px;
  background: var(--line-strong);
}

.constraint-row label {
  color: var(--muted);
  font-size: 8px;
}

.constraint-row span {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-top: 2px;
  color: var(--subtle);
  font-size: 8px;
}

.constraint-row input {
  width: 100%;
  min-width: 0;
  padding: 0;
  color: var(--ink);
  border: 0;
  outline: 0;
  background: transparent;
  font-size: 10px;
  font-weight: 600;
}

.asset-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.asset-row {
  display: grid;
  grid-template-columns: 30px 1fr 24px;
  align-items: center;
  gap: 7px;
  min-height: 38px;
  padding: 4px;
  border-radius: 7px;
}

.asset-row:hover {
  background: var(--surface);
}

.asset-thumb {
  width: 30px;
  height: 30px;
  display: grid;
  overflow: hidden;
  place-items: center;
  color: #a6a7a1;
  border-radius: 6px;
  background: #e7e7e2;
}

.asset-thumb img,
.picker-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.thumbnail-spinner {
  animation: thumbnail-spin 0.8s linear infinite;
}

@keyframes thumbnail-spin {
  to {
    transform: rotate(360deg);
  }
}

.asset-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.asset-copy strong {
  overflow: hidden;
  font-size: 10px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-copy small {
  margin-top: 2px;
  color: var(--subtle);
  font-size: 8px;
}

.folder-drop {
  width: 100%;
  min-height: 38px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  color: var(--muted);
  border: 1px dashed var(--line-strong);
  border-radius: 7px;
  background: var(--surface);
  cursor: pointer;
  font-size: 9px;
}

.folder-drop:hover {
  color: var(--accent-dark);
  border-color: #e2a08d;
  background: var(--accent-soft);
}

.workspace {
  position: absolute;
  top: 68px;
  right: 0;
  bottom: 0;
  left: 350px;
  overflow: auto;
  padding: 40px 44px;
  background-color: var(--canvas);
  background-image: radial-gradient(#d2d2cc 0.65px, transparent 0.65px);
  background-size: 16px 16px;
  transition: right 240ms cubic-bezier(0.22, 0.75, 0.2, 1);
}

.workspace.empty {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.workspace.empty .workspace-head {
  width: 100%;
  flex: 0 0 auto;
}

.workspace.empty .canvas {
  width: 100%;
  min-height: 0;
  flex: 1 1 auto;
}

.workspace.drawer-active {
  right: 444px;
}

.workspace-head {
  max-width: 1180px;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  margin: 0 auto 24px;
}

.workspace-head h1 {
  font-size: 28px;
  letter-spacing: -0.04em;
}

.workspace-head p {
  margin: 7px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.inline-alert {
  max-width: 1180px;
  min-height: 38px;
  display: flex;
  align-items: center;
  gap: 8px;
  margin: -10px auto 17px;
  padding: 0 12px;
  color: #8d5830;
  border: 1px solid #ecd0ae;
  border-radius: 9px;
  background: #fff7e7;
  font-size: 11px;
}

.canvas {
  max-width: 1180px;
  min-height: calc(100vh - 200px);
  margin: 0 auto;
  padding: 24px;
  border: 1px solid rgba(207, 207, 200, 0.76);
  border-radius: 18px;
  background: rgba(250, 250, 247, 0.86);
  box-shadow: var(--shadow-sm);
}

.canvas.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 28px;
  padding: 55px 24px;
}

.empty-canvas-copy {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 350px;
  text-align: center;
}

.empty-visual {
  position: relative;
  width: 86px;
  height: 58px;
  display: grid;
  margin-bottom: 18px;
  place-items: center;
  color: var(--accent);
  border: 1px solid var(--line-strong);
  border-radius: 12px;
  background: var(--paper);
  box-shadow: 0 10px 25px rgba(32, 32, 30, 0.08);
  transform: rotate(-3deg);
}

.film-line {
  position: absolute;
  inset: 7px;
  border-top: 2px dotted #d7d7d1;
  border-bottom: 2px dotted #d7d7d1;
}

.empty-canvas-copy h2 {
  margin: 3px 0 7px;
  font-size: 21px;
}

.empty-canvas-copy p {
  margin: 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.7;
}

.clip-grid {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(205px, 1fr));
  align-content: start;
  gap: 16px;
  container: clip-grid / inline-size;
}

.clip-card {
  position: relative;
  overflow: hidden;
  min-width: 0;
  border: 1px solid var(--line);
  border-radius: 13px;
  background: var(--paper);
  box-shadow: 0 4px 15px rgba(32, 32, 30, 0.04);
  cursor: pointer;
  transition: 180ms ease;
}

.clip-card:hover {
  border-color: #c7c7c1;
  box-shadow: 0 9px 24px rgba(32, 32, 30, 0.09);
  transform: translateY(-2px);
}

.clip-card.selected {
  border-color: var(--accent);
  box-shadow:
    0 0 0 2px rgba(242, 102, 69, 0.13),
    0 10px 28px rgba(32, 32, 30, 0.1);
}

.clip-preview {
  position: relative;
  aspect-ratio: 1.32;
  display: grid;
  overflow: hidden;
  place-items: center;
  color: #a2a39e;
  background: #e8e8e3;
}

.preview-grid {
  position: absolute;
  inset: 0;
  opacity: 0.65;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.28) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.28) 1px, transparent 1px);
  background-size: 25% 25%;
}

.preview-number {
  position: absolute;
  z-index: 1;
  top: 10px;
  left: 11px;
  color: #71726d;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.1em;
}

.preview-content {
  position: relative;
  z-index: 1;
  width: 48px;
  height: 48px;
  display: grid;
  place-items: center;
  color: white;
  border-radius: 50%;
  background: rgba(32, 32, 30, 0.82);
}

.preview-placeholder {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  font-size: 9px;
}

.clip-duration {
  position: absolute;
  z-index: 1;
  right: 8px;
  bottom: 8px;
  padding: 3px 6px;
  color: white;
  border-radius: 5px;
  background: rgba(32, 32, 30, 0.74);
  font-size: 8px;
  font-weight: 600;
}

.clip-card-body {
  padding: 12px;
}

.clip-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.clip-title strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.clip-title button {
  display: grid;
  flex: 0 0 auto;
  padding: 2px;
  place-items: center;
  color: var(--muted);
  border: 0;
  background: transparent;
  cursor: pointer;
}

.clip-time {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 7px;
  color: var(--subtle);
  font-size: 9px;
}

.clip-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 11px;
}

.clip-tags span {
  height: 22px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 7px;
  color: #5f605b;
  border-radius: 6px;
  background: #f0f0ec;
  font-size: 8px;
  font-weight: 600;
}

.clip-tags span.muted {
  color: #9a6a36;
  background: #fff3dd;
}

.clip-tags span.accent {
  color: var(--accent-dark);
  background: var(--accent-soft);
}

.clip-tags span.material-type.is-variable {
  color: var(--accent-dark);
  background: var(--accent-soft);
}

.clip-tags span.material-type.is-fixed {
  color: #47749e;
  background: #e7f0f7;
}

.selected-corner {
  position: absolute;
  top: 0;
  right: 0;
  width: 27px;
  height: 27px;
  display: grid;
  place-items: center;
  color: white;
  border-radius: 0 0 0 10px;
  background: var(--accent);
}

.add-clip-card {
  min-height: 225px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 20px;
  color: var(--muted);
  border: 1.5px dashed #c6c6c0;
  border-radius: 13px;
  background: rgba(255, 255, 255, 0.58);
  cursor: pointer;
  transition: 180ms ease;
}

.add-clip-card:hover {
  color: var(--accent-dark);
  border-color: var(--accent);
  background: #fff8f5;
  transform: translateY(-2px);
}

.canvas.empty .add-clip-card {
  --empty-card-width: 100%;
  width: 100%;
  max-width: var(--empty-card-width);
  grid-column: 1 / -1;
  justify-self: center;
}

.canvas.empty .add-clip-card:hover {
  transform: translateY(-2px);
}

@container clip-grid (min-width: 426px) {
  .canvas.empty .add-clip-card {
    --empty-card-width: calc((100cqw - 16px) / 2);
  }
}

@container clip-grid (min-width: 647px) {
  .canvas.empty .add-clip-card {
    --empty-card-width: calc((100cqw - 32px) / 3);
  }
}

@container clip-grid (min-width: 868px) {
  .canvas.empty .add-clip-card {
    --empty-card-width: calc((100cqw - 48px) / 4);
  }
}

@container clip-grid (min-width: 1089px) {
  .canvas.empty .add-clip-card {
    --empty-card-width: calc((100cqw - 64px) / 5);
  }
}

.add-clip-card > span {
  width: 45px;
  height: 45px;
  display: grid;
  margin-bottom: 3px;
  place-items: center;
  color: white;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 7px 18px rgba(242, 102, 69, 0.25);
}

.add-clip-card strong {
  color: var(--ink);
  font-size: 12px;
}

.add-clip-card small {
  color: var(--subtle);
  font-size: 8px;
}

.sequence-summary {
  min-width: 0;
  margin-top: 27px;
  padding-top: 18px;
  border-top: 1px solid var(--line);
}

.sequence-summary > div:first-child {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 10px;
  color: var(--muted);
  font-size: 10px;
  font-weight: 600;
}

.sequence-line {
  min-height: 32px;
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  padding: 3px;
  border-radius: 8px;
  background: #e3e3de;
}

.sequence-line span {
  min-width: 18px;
  min-height: 26px;
  display: grid;
  place-items: center;
  color: #898a84;
  border-radius: 5px;
  background: #f8f8f5;
  cursor: pointer;
  font-size: 8px;
  font-weight: 700;
}

.sequence-line span:hover,
.sequence-line span.active {
  color: white;
  background: var(--accent);
}

.sequence-tracks-scroller {
  margin-top: 12px;
  overflow-x: auto;
  border: 1px solid var(--line-strong);
  border-radius: 10px;
  background: #ecece7;
  scrollbar-width: thin;
  scrollbar-color: #c6c7c0 transparent;
}

.sequence-tracks {
  min-width: 100%;
  display: grid;
  gap: 5px;
  padding: 12px;
}

.sequence-timeline-ruler-row {
  height: 26px;
  min-width: 0;
  display: grid;
  grid-template-columns: 112px minmax(0, 1fr);
}

.sequence-timeline-ruler-row > span {
  position: sticky;
  z-index: 6;
  left: 0;
  border-right: 1px solid var(--line);
  background: #ecece7;
}

.sequence-timeline-ruler {
  position: relative;
  height: 26px;
  overflow: hidden;
  border-top: 1px solid var(--line);
  border-right: 1px solid var(--line);
  border-bottom: 1px solid var(--line);
  background: #f4f4f0;
}

.sequence-timeline-ruler .timeline-ruler-tick {
  position: absolute;
  top: 0;
  width: 1px;
  background: #b9bab4;
  pointer-events: none;
}

.sequence-timeline-ruler .timeline-ruler-tick.is-minor {
  height: 6px;
  background: #d0d1cb;
}

.sequence-timeline-ruler .timeline-ruler-tick.is-major {
  height: 11px;
}

.sequence-timeline-ruler .timeline-ruler-label {
  position: absolute;
  top: 12px;
  color: #74756f;
  font-family: 'JetBrains Mono', monospace;
  font-size: 8px;
  font-weight: 700;
  line-height: 10px;
  white-space: nowrap;
  pointer-events: none;
  transform: translateX(-50%);
}

.sequence-timeline-ruler .timeline-ruler-label.is-start {
  transform: none;
}

.sequence-timeline-ruler .timeline-ruler-label.is-end {
  transform: translateX(-100%);
}

.sequence-track-row {
  height: 34px;
  min-width: 0;
  display: grid;
  grid-template-columns: 112px minmax(0, 1fr);
  overflow: visible;
  border: 1px solid var(--line);
  border-radius: 5px;
  background: #fafaf7;
}

.sequence-track-label {
  position: sticky;
  z-index: 6;
  left: 0;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 0 9px;
  color: #62635e;
  border-right: 1px solid var(--line);
  background: #fbfbf9;
  font-size: 9px;
  white-space: nowrap;
}

.sequence-track-label span {
  width: 20px;
  height: 20px;
  display: grid;
  flex: 0 0 20px;
  place-items: center;
  color: var(--accent-dark);
  border-radius: 4px;
  background: var(--accent-soft);
  font-size: 8px;
  font-weight: 800;
}

.sequence-track-label strong {
  overflow: hidden;
  font-size: 9px;
  text-overflow: ellipsis;
}

.sequence-track-lane {
  position: relative;
  min-width: 0;
  overflow: hidden;
  background:
    linear-gradient(90deg, rgba(32, 32, 30, 0.05) 1px, transparent 1px),
    #f1f1ed;
  background-size: 40px 100%;
}

.sequence-track-clip {
  position: absolute;
  top: 3px;
  bottom: 3px;
  min-width: 28px;
  overflow: hidden;
  padding: 0 7px;
  color: #4f504b;
  border-radius: 3px;
  font-size: 8px;
  font-weight: 700;
  line-height: 26px;
  text-align: left;
  text-overflow: ellipsis;
  text-shadow: none;
  white-space: nowrap;
}

button.sequence-track-clip {
  display: flex;
  align-items: center;
  gap: 5px;
  border: 1px solid #e7aa95;
  background: #f9e8e1;
  cursor: pointer;
}

.sequence-track-clip-copy {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sequence-transition-bridge {
  position: absolute;
  z-index: 4;
  top: 3px;
  bottom: 3px;
  min-width: 10px;
  overflow: hidden;
  padding: 0;
  border: 1px solid #9565aa;
  border-radius: 3px;
  background:
    linear-gradient(
        135deg,
        transparent 46%,
        #a978bc 47%,
        #a978bc 53%,
        transparent 54%
      )
      left / 50% 100% no-repeat,
    linear-gradient(
        45deg,
        transparent 46%,
        #a978bc 47%,
        #a978bc 53%,
        transparent 54%
      )
      right / 50% 100% no-repeat,
    #f3e8f7;
  box-shadow: 0 1px 4px rgba(92, 53, 109, 0.2);
  cursor: pointer;
  transform: translateX(-50%);
}

.sequence-transition-bridge > span {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 50%;
  width: 1px;
  background: #815294;
  transform: translateX(-50%);
}

.sequence-transition-bridge:hover,
.sequence-transition-bridge.active {
  border-color: #684078;
  background-color: #e8d3ef;
  box-shadow: 0 0 0 2px rgba(149, 101, 170, 0.22);
}

.sequence-track-clip.is-full {
  right: 3px;
  left: 3px;
}

.sequence-track-clip.is-transition {
  border: 1px solid #cfb1dd;
  background: #f1e8f5;
}

.sequence-track-clip.is-fixed-material {
  border: 1px solid #abc6df;
  background: #e7f0f7;
}

.sequence-track-clip.is-variable-material {
  border: 1px solid #e7aa95;
  background: #f9e8e1;
}

.sequence-track-clip.is-audio {
  border: 1px solid #9bcdb5;
  background: #e5f3ec;
}

.sequence-track-clip.is-recording {
  border: 1px solid #ddaebf;
  background: #f6e6ec;
}

.sequence-track-clip.is-empty {
  color: #9a9b95;
  border-style: dashed;
  background: #f8f8f5;
  text-shadow: none;
}

button.sequence-track-clip:hover,
button.sequence-track-clip.active {
  z-index: 2;
  color: white;
  border-color: var(--accent-dark);
  background: var(--accent);
  box-shadow: 0 0 0 1px rgba(242, 102, 69, 0.2);
}

.clip-drawer {
  position: absolute;
  z-index: 40;
  top: 80px;
  right: 12px;
  bottom: 12px;
  width: 420px;
  display: grid;
  grid-template-rows: auto 1fr auto;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--paper);
  box-shadow: var(--shadow-lg);
}

.drawer-header {
  min-height: 72px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 15px 18px;
  border-bottom: 1px solid var(--line);
}

.drawer-header h2 {
  max-width: 310px;
  overflow: hidden;
  font-size: 16px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-scroll {
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: #d2d2cc transparent;
}

.drawer-section {
  padding: 19px 18px;
  border-bottom: 1px solid var(--line);
}

.drawer-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 15px;
}

.drawer-section-title.with-action,
.drawer-section-title.with-toggle {
  justify-content: space-between;
}

.drawer-section-title.with-action > div,
.drawer-section-title.with-toggle > div {
  display: flex;
  align-items: center;
  gap: 8px;
}

.drawer-section-title h3 {
  margin: 0;
  font-size: 13px;
}

.drawer-section-title em {
  min-width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  color: var(--muted);
  border-radius: 5px;
  background: #eeeeea;
  font-size: 8px;
  font-style: normal;
}

.step-number,
.modal-section-head > div > span {
  color: var(--accent);
  font-size: 8px;
  font-weight: 700;
  letter-spacing: 0.06em;
}

.field-note {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: -3px 0 0;
  color: var(--subtle);
  font-size: 9px;
}

.mini-button {
  height: 28px;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 9px;
  color: var(--accent-dark);
  border: 1px solid #f0c7bb;
  border-radius: 7px;
  background: var(--accent-soft);
  cursor: pointer;
  font-size: 9px;
  font-weight: 600;
}

.area-manager-entry {
  width: 100%;
  min-height: 62px;
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) 16px;
  align-items: center;
  gap: 10px;
  padding: 10px;
  color: var(--muted);
  text-align: left;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--surface);
  cursor: pointer;
}

.area-manager-entry:hover {
  color: var(--accent-dark);
  border-color: #e3a18e;
  background: var(--accent-soft);
}

.area-manager-entry > span {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.area-manager-entry strong {
  color: var(--ink);
  font-size: 10px;
}

.area-manager-entry small {
  color: var(--subtle);
  font-size: 8px;
}

.area-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.area-item {
  min-height: 51px;
  display: grid;
  grid-template-columns: 30px 1fr auto 25px 15px;
  align-items: center;
  gap: 9px;
  padding: 6px 9px 6px 6px;
  text-align: left;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--surface);
  cursor: pointer;
}

.area-delete:hover {
  color: var(--red);
  background: #fff0ee;
}

.area-item:hover {
  border-color: #c9c9c3;
  background: var(--paper);
}

.area-item.active {
  border-color: #e3a18e;
  background: var(--accent-soft);
  box-shadow: inset 3px 0 var(--accent);
}

.area-index {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  color: #777872;
  border-radius: 7px;
  background: #e7e7e2;
  font-size: 8px;
  font-weight: 700;
}

.area-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.area-main strong {
  overflow: hidden;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.area-main small {
  margin-top: 3px;
  color: var(--subtle);
  font-size: 8px;
}

.area-state {
  padding: 3px 5px;
  color: var(--green);
  border-radius: 4px;
  background: #eaf7f0;
  font-size: 7px;
  font-weight: 600;
}

.area-state.empty {
  color: #a06f35;
  background: #fff2dc;
}

.empty-block {
  width: 100%;
  min-height: 95px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  color: var(--subtle);
  border: 1px dashed var(--line-strong);
  border-radius: 10px;
  background: var(--surface);
  cursor: pointer;
}

.empty-block:hover {
  color: var(--accent-dark);
  border-color: #e3a18e;
  background: var(--accent-soft);
}

.empty-block strong {
  color: #5b5c57;
  font-size: 10px;
}

.empty-block span {
  font-size: 8px;
}

.empty-block.compact {
  min-height: 78px;
}

.switch {
  position: relative;
  display: block;
  width: 34px;
  height: 20px;
}

.switch input {
  position: absolute;
  opacity: 0;
}

.switch span {
  position: absolute;
  inset: 0;
  border-radius: 10px;
  background: #d9d9d3;
  cursor: pointer;
  transition: 160ms ease;
}

.switch span::after {
  content: '';
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
  transition: 160ms ease;
}

.switch input:checked + span {
  background: var(--accent);
}

.switch input:checked + span::after {
  transform: translateX(14px);
}

.transition-panel {
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--surface);
}

.disabled-hint {
  margin: 0;
  color: var(--subtle);
  font-size: 9px;
}

.subtitle-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.subtitle-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 28px;
  align-items: center;
  gap: 3px;
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--surface);
}

.subtitle-row {
  min-width: 0;
  min-height: 43px;
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 3px;
  text-align: left;
  border: 0;
  border-radius: 7px;
  background: transparent;
  cursor: pointer;
}

.subtitle-row:hover {
  background: var(--paper);
}

.subtitle-row-index {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  color: var(--accent-dark);
  border-radius: 7px;
  background: var(--accent-soft);
}

.subtitle-row-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.subtitle-row-main strong,
.subtitle-row-main small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-row-main strong {
  font-size: 10px;
}

.subtitle-row-main small {
  margin-top: 3px;
  color: var(--subtle);
  font-size: 7px;
}

.subtitle-row > svg {
  color: var(--subtle);
}

.subtitle-head {
  min-height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 7px;
}

.subtitle-head > button:first-child {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.subtitle-head strong {
  font-size: 10px;
}

.subtitle-head span {
  padding: 2px 5px;
  color: var(--muted);
  border-radius: 4px;
  background: #e7e7e2;
  font-size: 7px;
}

.subtitle-body {
  padding: 11px;
  border-top: 1px solid var(--line);
  background: var(--paper);
}

.segmented {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 3px;
  margin-bottom: 12px;
  padding: 3px;
  border-radius: 8px;
  background: #ecece7;
}

.segmented.three {
  grid-template-columns: repeat(3, 1fr);
  margin: 0;
}

.clip-material-segmented {
  margin-bottom: 0;
}

.segmented button {
  height: 29px;
  padding: 0 7px;
  color: var(--muted);
  border: 0;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  font-size: 9px;
  font-weight: 600;
}

.segmented button.active {
  color: var(--ink);
  background: var(--paper);
  box-shadow: 0 1px 5px rgba(32, 32, 30, 0.08);
}

.color-input {
  height: 36px;
  display: grid;
  grid-template-columns: 31px 1fr;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: var(--paper);
}

.color-input input[type='color'] {
  width: 31px;
  height: 36px;
  padding: 4px;
  border: 0;
  border-right: 1px solid var(--line);
  border-radius: 0;
}

.color-input input[type='text'] {
  height: 34px;
  border: 0;
  border-radius: 0;
}

.subtitle-modal {
  width: min(900px, 100%);
}

.subtitle-settings-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.subtitle-preview-section {
  margin-top: 12px;
}

.subtitle-screen-preview {
  container-type: inline-size;
}

.subtitle-preview-text {
  position: absolute;
  z-index: 2;
  max-width: calc(100% - 16px);
  padding: 3px 6px;
  line-height: 1.25;
  font-weight: 600;
  text-align: center;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  border: 1px dashed transparent;
  border-radius: 4px;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
  transform: translate(-50%, -50%);
  cursor: grab;
  user-select: none;
  touch-action: none;
}

.subtitle-preview-text:hover {
  border-color: rgba(255, 255, 255, 0.5);
  background: rgba(0, 0, 0, 0.18);
}

.subtitle-preview-text:active {
  cursor: grabbing;
}

.subtitle-preview-text.placeholder {
  opacity: 0.55;
  font-style: italic;
}

.drawer-footer {
  min-height: 63px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid var(--line);
  background: #fbfbf9;
}

.drawer-move {
  display: flex;
  gap: 4px;
}

.drawer-move button {
  height: 31px;
  padding: 0 10px;
  color: var(--muted);
  border: 1px solid var(--line);
  border-radius: 7px;
  background: var(--paper);
  cursor: pointer;
  font-size: 9px;
}

.drawer-move button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.modal-backdrop {
  position: fixed;
  z-index: 400;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 28px;
  color: #20201e;
  background: rgba(28, 28, 26, 0.46);
  backdrop-filter: blur(3px);
  font-family: 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  --ink: #20201e;
  --muted: #777872;
  --subtle: #a2a39d;
  --line: #e3e3de;
  --line-strong: #d5d5cf;
  --paper: #ffffff;
  --surface: #f8f8f5;
  --canvas: #efefeb;
  --accent: #f26645;
  --accent-dark: #dc4d2c;
  --accent-soft: #fff0eb;
  --yellow: #f0bb4c;
  --green: #319b68;
  --red: #d64a43;
  --shadow-sm: 0 4px 16px rgba(32, 32, 30, 0.06);
  --shadow-lg: 0 22px 70px rgba(32, 32, 30, 0.18);
}

.area-modal {
  width: min(1320px, 100%);
  max-height: min(820px, calc(100vh - 56px));
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) auto auto;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.8);
  border-radius: 17px;
  background: var(--paper);
  box-shadow: 0 28px 90px rgba(18, 18, 16, 0.3);
}

.modal-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 20px 22px 17px;
  border-bottom: 1px solid var(--line);
}

.modal-header h2 {
  font-size: 20px;
  letter-spacing: -0.02em;
}

.modal-header p {
  margin: 5px 0 0;
  color: var(--muted);
  font-size: 10px;
}

.area-track-upload {
  min-width: 0;
  min-height: 58px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 9px 22px;
  background: #fbfbf9;
}

.area-track-upload-required {
  border-bottom: 1px solid var(--line);
  background: linear-gradient(90deg, var(--accent-soft), #fbfbf9 48%);
}

.area-track-upload-optional {
  border-top: 1px solid var(--line);
}

.area-track-upload-copy {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
}

.area-track-upload-copy > div {
  min-width: 0;
}

.area-track-upload-icon {
  width: 32px;
  height: 32px;
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  color: var(--accent-dark);
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--paper);
}

.area-track-upload-copy strong {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
}

.area-track-upload-copy strong em {
  color: var(--accent-dark);
  font-size: 8px;
  font-style: normal;
}

.area-track-upload-copy strong em.optional {
  color: var(--muted);
}

.area-track-upload-copy div > span {
  display: block;
  overflow: hidden;
  max-width: 520px;
  margin-top: 2px;
  color: var(--subtle);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.area-track-upload-button {
  min-height: 31px;
  padding: 6px 10px;
  font-size: 10px;
  white-space: nowrap;
}

.modal-body {
  overflow-y: auto;
  padding: 20px 22px;
  scrollbar-width: thin;
  scrollbar-color: #d2d2cc transparent;
}

.area-modal-body {
  min-height: 0;
  overflow: hidden;
  padding: 0;
}

.area-editor-layout {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
}

.area-asset-pane {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-columns: 50px minmax(0, 1fr);
  border-right: 1px solid var(--line);
  background: var(--surface);
}

.asset-folder-tabs {
  min-width: 0;
  overflow-y: auto;
  padding: 9px 0;
  border-right: 1px solid var(--line);
  background: #eeeeea;
  scrollbar-width: thin;
}

.asset-folder-tab {
  width: 100%;
  min-height: 27px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 2px;
  padding: 4px 4px 4px 7px;
  color: var(--muted);
  text-align: left;
  border: 0;
  border-radius: 0 6px 6px 0;
  background: transparent;
  cursor: pointer;
}

.asset-folder-tab + .asset-folder-tab {
  margin-top: 2px;
}

.asset-folder-tab:hover {
  color: var(--ink);
  background: rgba(255, 255, 255, 0.55);
}

.asset-folder-tab.active {
  color: var(--accent-dark);
  background: var(--paper);
  box-shadow:
    inset 3px 0 var(--accent),
    1px 0 var(--paper);
}

.asset-folder-tab span {
  min-width: 0;
  overflow: hidden;
  font-size: 9px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-folder-tab small {
  min-width: 13px;
  height: 13px;
  display: grid;
  place-items: center;
  color: inherit;
  border-radius: 4px;
  background: rgba(32, 32, 30, 0.06);
  font-size: 6px;
}

.asset-picker-panel {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 14px 9px;
}

.asset-picker-panel .modal-section-head {
  flex: 0 0 auto;
}

.area-main-pane {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 320px);
  grid-template-rows: auto minmax(0, 1fr);
  align-items: start;
  gap: 10px 12px;
  overflow-y: auto;
  overflow-anchor: none;
  padding: 14px 16px;
  scrollbar-width: thin;
  scrollbar-color: #d2d2cc transparent;
}

.area-dialog-manager {
  grid-column: 1 / -1;
  grid-row: 1;
  margin-bottom: 8px;
  padding: 8px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: #fbfbf9;
}

.area-dialog-manager-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}

.area-dialog-manager-head > div {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 7px;
}

.area-dialog-manager-head strong {
  font-size: 11px;
}

.area-dialog-manager-head small {
  color: var(--subtle);
  font-size: 8px;
}

.area-dialog-list .area-item {
  min-height: 43px;
  gap: 7px;
  padding: 4px 7px 4px 4px;
  background: var(--paper);
}

.area-dialog-list {
  gap: 4px;
}

.area-dialog-list .area-index {
  width: 27px;
  height: 27px;
}

.area-dialog-list .area-item.active {
  background: var(--accent-soft);
}

.modal-section {
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--surface);
}

.modal-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 13px;
}

.modal-section-head > div {
  display: flex;
  align-items: center;
  gap: 8px;
}

.modal-section-head h3 {
  margin: 0;
  font-size: 12px;
}

.modal-section-head small {
  color: var(--subtle);
  font-size: 8px;
}

.modal-section-toggle {
  width: 100%;
  padding: 0;
  color: inherit;
  text-align: left;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.modal-section-toggle:hover h3 {
  color: var(--accent);
}

.modal-section-toggle > svg,
.modal-section-toggle-meta > svg {
  flex: 0 0 auto;
  color: var(--subtle);
}

.modal-section-toggle-meta {
  display: flex;
  align-items: center;
  gap: 7px;
}

.modal-section-collapse {
  display: grid;
  grid-template-rows: 1fr;
}

.modal-section-collapse-inner {
  min-height: 0;
  overflow: hidden;
}

.asset-picker {
  max-height: 175px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  overflow-y: auto;
  padding-right: 3px;
  scrollbar-width: thin;
}

.asset-picker-panel .asset-picker {
  min-height: 0;
  max-height: none;
  flex: 1 1 0;
  grid-auto-rows: max-content;
  align-content: start;
  overscroll-behavior: contain;
}

.asset-picker > button {
  min-width: 0;
  height: max-content;
  align-self: start;
  overflow: hidden;
  padding: 5px;
  text-align: left;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--paper);
  cursor: pointer;
}

.asset-picker > button[draggable='true'] {
  cursor: grab;
}

.asset-picker > button[draggable='true']:active {
  cursor: grabbing;
}

.asset-picker > button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.asset-picker > button:hover,
.asset-picker > button.selected {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px rgba(242, 102, 69, 0.1);
}

.picker-thumb {
  position: relative;
  aspect-ratio: 1.7;
  display: grid;
  overflow: hidden;
  place-items: center;
  margin-bottom: 6px;
  color: #a4a59f;
  border-radius: 6px;
  background: #e7e7e2;
}

.picker-thumb > span {
  position: absolute;
  top: 5px;
  right: 5px;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  color: white;
  border-radius: 50%;
  background: var(--accent);
}

.asset-picker strong,
.asset-picker small {
  display: block;
  overflow: hidden;
  padding: 0 2px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-picker strong {
  font-size: 9px;
}

.asset-picker small {
  margin-top: 2px;
  color: var(--subtle);
  font-size: 7px;
}

.no-assets {
  min-height: 75px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 11px;
  color: var(--subtle);
  border: 1px dashed var(--line-strong);
  border-radius: 9px;
}

.no-assets > div {
  display: flex;
  flex-direction: column;
}

.no-assets strong {
  color: #5c5d58;
  font-size: 10px;
}

.no-assets span {
  margin-top: 3px;
  font-size: 8px;
}

.asset-picker-panel .no-assets {
  min-height: 0;
  flex: 1;
  flex-direction: column;
  padding: 14px 8px;
  text-align: center;
}

.asset-picker-panel .no-assets > div {
  align-items: center;
}

.modal-columns {
  display: grid;
  grid-template-columns: 1fr 1fr;
  align-items: start;
  gap: 12px;
  margin-top: 12px;
}

.area-main-pane .modal-columns {
  grid-template-columns: minmax(0, 1fr);
  gap: 6px;
  margin-top: 0;
}

.area-main-pane.has-area-draft .area-dialog-manager {
  grid-column: 1;
}

.area-settings-stack {
  display: contents;
}

.area-settings-stack > .canvas-preview-section {
  grid-column: 1;
  grid-row: 2;
  margin-top: 0;
}

.area-settings-stack > .modal-columns {
  grid-column: 2;
  grid-row: 1 / span 2;
  align-self: start;
  padding-left: 9px;
  border-left: 1px solid var(--line);
}

.area-main-pane .modal-section {
  padding: 8px;
  border-radius: 8px;
}

.area-main-pane .modal-section.bound {
  background: #f2f2ee;
}

.beauty-modal-section {
  grid-column: 1 / -1;
}

.area-beauty-settings {
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.area-beauty-settings:disabled {
  opacity: 0.62;
}

.area-beauty-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
  color: var(--subtle);
  font-size: 9px;
}

.area-beauty-reset {
  height: 23px;
  padding: 0 7px;
  flex: 0 0 auto;
  color: var(--accent-dark);
  border: 1px solid #f1c5b6;
  border-radius: 6px;
  background: #fff7f3;
  cursor: pointer;
  font-size: 9px;
  font-weight: 700;
}

.area-beauty-reset:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.area-inspector-choice-row {
  min-height: 36px;
  display: grid;
  grid-template-columns: 62px minmax(0, 1fr);
  align-items: center;
  gap: 7px;
  padding: 4px 2px;
  border-bottom: 1px solid var(--line);
}

.area-inspector-choice-row > span,
.area-inspector-label,
.area-beauty-skin-head > span,
.area-beauty-switches > label > span:first-child {
  color: #5e5f59;
  font-size: 9px;
  font-weight: 600;
  white-space: nowrap;
}

.area-inspector-controls {
  display: grid;
}

.area-inspector-row,
.area-beauty-control {
  min-width: 0;
  min-height: 35px;
  display: grid;
  grid-template-columns: 62px minmax(60px, 1fr) 58px;
  align-items: center;
  gap: 7px;
  padding: 4px 2px;
  border-bottom: 1px solid var(--line);
}

.area-inspector-controls > .area-inspector-row:last-child {
  border-bottom: 0;
}

.transition-duration-control {
  grid-template-columns: 62px minmax(60px, 1fr) 72px;
}

.area-inspector-pair-row {
  grid-template-columns: 62px minmax(0, 1fr);
}

.area-inspector-pair {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.area-inspector-spacer {
  min-width: 0;
}

.area-inspector-value {
  min-width: 0;
  height: 25px;
  display: flex;
  align-items: center;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  background: var(--paper);
}

.area-inspector-value:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.area-inspector-value input {
  width: 100%;
  min-width: 0;
  height: 23px;
  padding: 0 5px;
  color: var(--ink);
  border: 0;
  outline: 0;
  background: transparent;
  font-size: 9px;
  text-align: center;
}

.area-inspector-value small {
  padding-right: 5px;
  color: var(--subtle);
  font-size: 8px;
}

.area-inspector-value.has-prefix small {
  padding-right: 0;
  padding-left: 5px;
  color: var(--accent-dark);
  font-weight: 700;
}

.area-inspector-range {
  width: 100%;
  height: 18px;
  margin: 0;
  padding: 0;
  border: 0;
  outline: 0;
  appearance: none;
  background: transparent;
  cursor: pointer;
}

.area-inspector-range::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 999px;
  background: var(--line-strong);
}

.area-inspector-range::-webkit-slider-thumb {
  width: 11px;
  height: 11px;
  margin-top: -4px;
  border: 2px solid white;
  border-radius: 50%;
  appearance: none;
  background: var(--accent);
  box-shadow: 0 1px 4px rgba(220, 77, 44, 0.28);
}

.area-inspector-range:focus-visible::-webkit-slider-thumb {
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.area-inspector-range:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.area-beauty-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  border-top: 1px solid var(--line);
}

.area-beauty-control {
  grid-template-columns: 62px minmax(60px, 1fr) 58px;
}

.area-beauty-control.is-select {
  grid-template-columns: 62px minmax(0, 1fr);
}

.area-beauty-control select {
  width: 100%;
  height: 25px;
  padding: 0 8px;
  color: var(--ink);
  border: 1px solid var(--line-strong);
  outline: none;
  border-radius: 6px;
  background: var(--paper);
  font-size: 9px;
}

.area-beauty-control select:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.area-beauty-skin {
  min-width: 0;
  border-bottom: 1px solid var(--line);
}

.area-beauty-skin-head {
  display: flex;
  min-height: 35px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 2px;
}

.area-skin-tone-options {
  display: flex;
  align-items: center;
  gap: 8px;
}

.area-skin-tone-option {
  position: relative;
  width: 21px;
  height: 21px;
  padding: 0;
  border: 2px solid var(--paper);
  border-radius: 50%;
  outline: 1px solid var(--line-strong);
  background: var(--swatch-color);
  cursor: pointer;
}

.area-skin-tone-option:hover,
.area-skin-tone-option.active {
  outline-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.area-skin-tone-option.off {
  background: var(--surface);
}

.area-skin-tone-option.off::after {
  position: absolute;
  top: 8px;
  left: 3px;
  width: 14px;
  height: 1px;
  background: var(--subtle);
  content: '';
  transform: rotate(45deg);
}

.area-skin-adjustments {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  border-top: 1px solid var(--line);
}

.area-skin-adjustments .area-beauty-control {
  padding: 4px 2px;
  border: 0;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
}

.area-skin-adjustments .area-beauty-control:last-child {
  border-bottom: 0;
}

.area-beauty-switches {
  display: grid;
  align-content: center;
}

.area-beauty-switches > label {
  display: flex;
  min-height: 35px;
  align-items: center;
  justify-content: space-between;
  gap: 7px;
  padding: 4px 2px;
  border-bottom: 1px solid var(--line);
}

.area-beauty-switches > label:last-child {
  border-bottom: 0;
}

.area-main-pane .modal-section.bound input:disabled,
.area-main-pane .modal-section.bound button:disabled {
  opacity: 0.62;
  cursor: not-allowed;
}

.area-main-pane .segmented button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.area-main-pane .modal-section-head {
  margin-bottom: 6px;
}

.area-main-pane .modal-section-toggle {
  padding: 6px 8px;
  color: #3f3f3a;
  border: 1px solid #ffd6c8;
  border-radius: 7px;
  background: var(--accent-soft);
  transition:
    background-color 160ms ease,
    border-color 160ms ease;
}

.area-main-pane .modal-section-toggle:hover {
  border-color: #f8b9a4;
  background: #ffe4da;
}

.area-main-pane .modal-section-toggle > svg,
.area-main-pane .modal-section-toggle-meta > svg {
  color: var(--accent);
}

.area-main-pane .modal-section-toggle[aria-expanded='false'] {
  margin-bottom: 0;
}

.area-main-pane .field {
  margin-bottom: 8px;
}

.area-main-pane .field label {
  margin-bottom: 4px;
}

.area-main-pane .field input,
.area-main-pane .field select {
  height: 32px;
}

.area-main-pane .field-grid,
.area-main-pane .position-grid {
  gap: 7px;
}

.area-main-pane .area-opacity-field {
  margin-top: 2px;
}

.area-editor-empty {
  grid-column: 1 / -1;
  min-height: 280px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 7px;
  color: var(--subtle);
  border: 1px dashed var(--line-strong);
  border-radius: 10px;
  background: var(--surface);
  text-align: center;
}

.area-editor-empty strong {
  color: var(--ink);
  font-size: 11px;
}

.area-editor-empty span {
  font-size: 9px;
}

.position-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.canvas-preview-section {
  display: block;
  margin-top: 12px;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: #292926;
}

.area-main-pane .modal-section-toggle h3 {
  font-size: 11px;
}

.screen-preview {
  position: relative;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  touch-action: none;
  border-radius: 6px;
  background-color: #171715;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.05) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.05) 1px, transparent 1px);
  background-size: 25% 25%;
}

.screen-preview.asset-drag-over {
  box-shadow: inset 0 0 0 2px var(--accent);
}

.screen-preview.asset-drag-over::after {
  content: '松开以创建画面区域';
  position: absolute;
  z-index: 10;
  inset: 8px;
  display: grid;
  place-items: center;
  pointer-events: none;
  color: white;
  border: 1px dashed rgba(255, 255, 255, 0.8);
  border-radius: 5px;
  background: rgba(242, 102, 69, 0.36);
  font-size: 10px;
  font-weight: 600;
}

.screen-safe {
  position: absolute;
  inset: 10%;
  border: 1px dashed rgba(255, 255, 255, 0.16);
}

.area-rect {
  position: absolute;
  display: grid;
  place-items: center;
  color: white;
  border: 1px solid var(--accent);
  background: rgba(242, 102, 69, 0.28);
  box-shadow: 0 0 0 1px rgba(242, 102, 69, 0.2);
  cursor: move;
  user-select: none;
  touch-action: none;
}

.area-preview-image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  object-fit: cover;
  user-select: none;
}

.area-rect-active .area-preview-image {
  opacity: 0.92;
}

.area-rect-readonly .area-preview-image {
  opacity: 0.68;
  filter: saturate(0.72);
}

.area-rect-active {
  z-index: 2;
}

.area-rect-active.bound {
  cursor: default;
}

.area-rect.triptych {
  background: #11110f;
}

.area-rect-active:hover {
  background: rgba(242, 102, 69, 0.36);
}

.area-rect-readonly {
  z-index: 1;
  color: #d8dbe1;
  border-color: #89919f;
  border-style: dashed;
  background: rgba(137, 145, 159, 0.16);
  box-shadow: none;
  cursor: pointer;
}

.area-rect-readonly .area-size-label {
  color: #d8dbe1;
  background: rgba(52, 56, 63, 0.78);
}

.area-size-label {
  position: relative;
  z-index: 1;
  pointer-events: none;
  padding: 2px 4px;
  border-radius: 3px;
  background: rgba(32, 32, 30, 0.65);
  font-size: 7px;
  white-space: nowrap;
}

.area-resize-handle {
  position: absolute;
  z-index: 2;
  width: 10px;
  height: 10px;
  padding: 0;
  border: 1px solid white;
  border-radius: 2px;
  background: var(--accent);
  touch-action: none;
}

.area-resize-handle.handle-nw {
  top: -1px;
  left: -1px;
  cursor: nwse-resize;
}

.area-resize-handle.handle-ne {
  top: -1px;
  right: -1px;
  cursor: nesw-resize;
}

.area-resize-handle.handle-sw {
  bottom: -1px;
  left: -1px;
  cursor: nesw-resize;
}

.area-resize-handle.handle-se {
  right: -1px;
  bottom: -1px;
  cursor: nwse-resize;
}

.area-context-menu {
  position: fixed;
  z-index: 140;
  width: 136px;
  padding: 5px;
  color: var(--ink);
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--paper);
  box-shadow: 0 12px 36px rgba(24, 24, 22, 0.2);
  font-size: 10px;
}

.area-context-item {
  position: relative;
  min-height: 30px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 8px;
  border-radius: 5px;
  cursor: default;
}

.area-context-item:hover {
  color: var(--accent-dark);
  background: var(--accent-soft);
}

.area-context-action {
  width: 100%;
  color: inherit;
  text-align: left;
  border: 0;
  background: transparent;
  font: inherit;
  cursor: pointer;
}

.area-context-action:disabled {
  color: #b9bab5;
  cursor: not-allowed;
  background: transparent;
}

.area-context-submenu {
  position: absolute;
  top: -5px;
  left: 100%;
  width: 116px;
  display: none;
  padding: 5px;
  color: var(--ink);
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--paper);
  box-shadow: 0 12px 36px rgba(24, 24, 22, 0.2);
}

.area-context-item.has-submenu:hover .area-context-submenu {
  display: block;
}

.area-context-submenu button {
  width: 100%;
  height: 29px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 0 8px;
  color: inherit;
  text-align: left;
  border: 0;
  border-radius: 5px;
  background: transparent;
  cursor: pointer;
  font-size: 10px;
}

.area-context-submenu button:hover:not(:disabled) {
  color: var(--accent-dark);
  background: var(--accent-soft);
}

.area-context-submenu button:disabled {
  color: #b9bab5;
  cursor: not-allowed;
}

.modal-footer {
  min-height: 65px;
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  padding: 12px 22px;
  border-top: 1px solid var(--line);
  background: #fbfbf9;
}

.auto-save-note {
  margin-right: auto;
  color: var(--subtle);
  font-size: 9px;
}

.confirm-backdrop {
  z-index: 420;
}

.confirm-modal {
  width: min(380px, 100%);
  padding: 25px;
  border-radius: 15px;
  background: var(--paper);
  box-shadow: var(--shadow-lg);
  text-align: center;
}

.confirm-icon {
  width: 46px;
  height: 46px;
  display: grid;
  margin: 0 auto 13px;
  place-items: center;
  color: var(--red);
  border-radius: 50%;
  background: #fff0ee;
}

.confirm-modal h3 {
  margin: 0;
  font-size: 16px;
}

.confirm-modal p {
  margin: 9px 0 20px;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.65;
}

.confirm-modal > div:last-child {
  display: flex;
  justify-content: center;
  gap: 8px;
}

.toast {
  position: fixed;
  z-index: 450;
  top: 82px;
  left: 50%;
  min-height: 42px;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: min(440px, calc(100vw - 32px));
  padding: 0 14px;
  color: white;
  border-radius: 10px;
  background: #30312e;
  box-shadow: 0 10px 30px rgba(32, 32, 30, 0.2);
  font-size: 11px;
  transform: translateX(-50%);
}

.toast.success svg {
  color: #63d39b;
}

.toast.warning svg {
  color: #f2c25c;
}

.toast.error {
  background: #762f2a;
}

.drawer-enter-active,
.drawer-leave-active {
  transition: 240ms cubic-bezier(0.22, 0.75, 0.2, 1);
}

.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
  transform: translateX(35px);
}

.modal-enter-active,
.modal-leave-active,
.toast-enter-active,
.toast-leave-active {
  transition: 180ms ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .area-modal,
.modal-enter-from .confirm-modal,
.modal-leave-to .area-modal,
.modal-leave-to .confirm-modal {
  transform: translateY(10px) scale(0.985);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, -8px);
}

@media (max-width: 1180px) {
  .workspace.drawer-active {
    right: 0;
  }

  .clip-drawer {
    width: 410px;
    background: rgba(255, 255, 255, 0.98);
  }

  .workspace {
    padding-right: 30px;
    padding-left: 30px;
  }
}

@media (max-width: 860px) {
  .app-shell {
    overflow: auto;
  }

  .topbar {
    grid-template-columns: 1fr auto;
  }

  .brand {
    border-right: 0;
  }

  .topbar-center {
    display: none;
  }

  .sidebar {
    position: relative;
    top: 68px;
    width: 100%;
    max-height: none;
    overflow: visible;
    border-right: 0;
  }

  .workspace {
    position: relative;
    top: 68px;
    left: 0;
    min-height: 700px;
    overflow: visible;
  }

  .workspace.empty {
    overflow: visible;
  }

  .clip-drawer {
    top: 78px;
    right: 8px;
    bottom: 8px;
    width: min(420px, calc(100vw - 16px));
  }
}

@media (max-width: 760px) {
  .subtitle-settings-grid {
    grid-template-columns: 1fr;
  }

  .area-editor-layout {
    display: grid;
    grid-template-columns: 1fr;
    overflow-y: auto;
  }

  .area-asset-pane {
    min-height: 300px;
    max-height: 340px;
    grid-template-columns: 52px minmax(0, 1fr);
    border-right: 0;
    border-bottom: 1px solid var(--line);
  }

  .asset-folder-tabs {
    padding: 7px 0;
  }

  .asset-picker-panel {
    padding: 14px 12px;
  }

  .area-main-pane {
    min-height: auto;
    display: block;
    overflow: visible;
    padding: 18px;
  }

  .area-settings-stack {
    display: flex;
    flex-direction: column;
  }

  .area-settings-stack > .canvas-preview-section {
    order: 1;
  }

  .area-settings-stack > .modal-columns {
    order: 2;
    margin-top: 8px;
    padding-left: 0;
    border-left: 0;
  }
}

@media (max-width: 620px) {
  .topbar {
    height: 62px;
  }

  .brand {
    padding: 0 12px;
  }

  .brand > div:last-child span {
    display: none;
  }

  .topbar-actions {
    gap: 5px;
    padding-right: 8px;
  }

  .topbar-actions .button {
    min-height: 34px;
    padding: 0 9px;
    font-size: 10px;
  }

  .sidebar,
  .workspace {
    top: 62px;
  }

  .workspace {
    padding: 27px 14px;
  }

  .workspace-head {
    align-items: flex-start;
  }

  .canvas {
    padding: 14px;
  }

  .clip-grid {
    grid-template-columns: 1fr;
  }

  .canvas.empty .add-clip-card {
    max-width: 100%;
  }

  .clip-card,
  .add-clip-card {
    min-height: 0;
  }

  .modal-backdrop {
    padding: 8px;
  }

  .area-modal {
    max-height: calc(100vh - 16px);
  }

  .modal-columns {
    grid-template-columns: 1fr;
  }

  .asset-picker {
    grid-template-columns: repeat(2, 1fr);
  }

  .canvas-preview-section {
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
  }
}
</style>

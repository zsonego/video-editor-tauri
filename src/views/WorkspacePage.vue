<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import logoImage from "../assets/logo.png";

const emit = defineEmits(["logout"]);

const videoSource =
  "https://github.com/zsonego/video-editor-tauri/releases/download/new/video2.mp4";

const weddingImage =
  "https://lh3.googleusercontent.com/aida-public/AB6AXuAKUjxUCNitv9tXTzfgFJpJ4Ej2XBvFvC1QMHEvkCguRpbLnSBT-kk_vXIIoyD7N1vu54LtsFl8I-qrAPoQ5ugSt1UZmfUCbNYNGVgsNUoAepuaVksq5X2i4VxN6p5yZNN7-98lafAPrgTXJlKD7WRNAb90zS7OTDMthSpKGyVhyjdK3iFjr4wWg4esqt0UjogKVo_E-UDa7AgxhHefXqneVydxtp_jQThpAjL49uUw3p6I71h6jgJM4UVFvJNhRD7JEsqJepC6_ZD5";
const travelImage =
  "https://lh3.googleusercontent.com/aida-public/AB6AXuDt5PYLfvUtvuJfd9ZXljOhgs6n3G7R0iINSiXx-ZoXEu-JOruIUsel9dwYttKDKMJnsKyDopxdF1OY733OuzNsL3fzYyTIDqFUACrdIIv2WryUoF4T3fSxwuP0j8mZObr1sEQwYVdgKlIoermFPZEBOVTTSBzlsJ8xe_pFnMkrTTANjkAS3J7tgsoYud_mRfeEeHnCF8uJ4VIt6O-cmoH_30lPeXfZjAqGD3k7VhyUN2QIdI-_YCtH7HLHbJyCB7-YCGLmEaCFw9BH";
const previewImage =
  "https://lh3.googleusercontent.com/aida-public/AB6AXuAMcVYT2orNSDIC1Dv0--M8nRRmT3Z9u7QTh4ZauGBirOaFxvTQTX0Oy24AemimD6kAYfwyMZadW9d6ymXe8RbrOA9DCqlfmyjoC5BGRhUzB8FsE0ZUlTKOfEnXypm4GpzVoGQm_h4jb2P0z5ZY9vDEoO9eUqfGKS8PnooFtD4fxqwhKvd1QYIOTervYiOuZIO2e-_SOKMM9fGYOuiKIABVknNauRvRFZheB9iwOFViroat8J0NFhKjLczc5nerSjIT7iCRyk9Esps";

const categories = ["婚纱照唯美 (18)", "旅行Vlog (12)", "生日纪念 (15)", "记录生活 (20)"];
const recommendationCards = [
  {
    title: "婚礼亮点",
    subtitle: "18个片段 · 浪漫婚礼",
    meta: "浪漫婚礼 · 18个片段",
    image: weddingImage,
    badge: "热门",
  },
  {
    title: "旅行Vlog快剪",
    subtitle: "12个片段 · 环球旅行",
    meta: "环球旅行 · 12个片段",
    image: travelImage,
  },
];
const subtopics = [
  { title: "片头开启", count: 1, subtitle: "1个片段 · 唯美片头", material: 1, duration: "15s" },
  { title: "新娘备婚", count: 5, subtitle: "5个片段 · 记录点滴", material: 8, duration: "12s" },
];
const templateSegments = {
  婚礼亮点: [
    { name: "片段 1: 浪漫片头", shot: "全景 (Wide)", count: 2 },
    { name: "片段 2: 细节捕捉", shot: "特写 (Close-up)", count: 4 },
    { name: "片段 3: 情感高潮", shot: "中景 (Medium)", count: 3 },
    { name: "片段 4: 交换戒指", shot: "手部特写", count: 2 },
  ],
  旅行Vlog快剪: [
    { name: "片段 1: 出发准备", shot: "主观视角 (POV)", count: 3 },
    { name: "片段 2: 风光空镜", shot: "全景航拍", count: 5 },
    { name: "片段 3: 快节奏闪现", shot: "定格动画", count: 4 },
  ],
  片头开启: [{ name: "片段 1: 动态标题", shot: "文字特写", count: 1 }],
  新娘备婚: [
    { name: "片段 1: 晨间梳妆", shot: "侧脸特写", count: 2 },
    { name: "片段 2: 婚纱细节", shot: "慢动作特写", count: 3 },
    { name: "片段 3: 伴娘合影", shot: "中景", count: 3 },
  ],
};
const sourcePool = [
  { name: "视频 1", duration: "00:08", tag: "主镜头" },
  { name: "视频 2", duration: "00:12", tag: "特写" },
  { name: "视频 3", duration: "00:10", tag: "转场" },
  { name: "视频 4", duration: "00:15", tag: "氛围" },
  { name: "视频 5", duration: "00:09", tag: "补充" },
  { name: "视频 6", duration: "00:18", tag: "空镜" },
  { name: "视频 7", duration: "00:11", tag: "细节" },
  { name: "视频 8", duration: "00:14", tag: "收尾" },
];
const finishedGroups = [
  {
    name: "主线叙事",
    videos: [
      { name: "视频 1", time: "00:00 - 00:48" },
      { name: "视频 2", time: "00:48 - 01:36" },
    ],
  },
  {
    name: "氛围补充",
    videos: [
      { name: "视频 3", time: "01:36 - 02:42" },
      { name: "视频 4", time: "02:42 - 03:45" },
    ],
  },
];
const finishedVideos = [
  {
    id: "finished-wedding-highlight",
    title: "婚礼亮点_最终成片.mp4",
    displayName: "婚礼亮点_最终成片",
    image: weddingImage,
    duration: "03:45",
    date: "2026-05-14",
  },
  {
    id: "finished-travel-vlog",
    title: "旅行Vlog_快剪成片.mp4",
    displayName: "旅行Vlog_快剪成片",
    image: travelImage,
    duration: "01:28",
    date: "2026-05-13",
  },
];

const activeCategory = ref(0);
const sidebarHidden = ref(true);
const currentViewState = ref("subtopics");
const mainMode = ref("grid");
const activeTemplateName = ref("");
const previewTitle = ref("新娘备婚");
const previewSubtitle = ref("5个片段 · 记录点滴");
const previewModalVisible = ref(false);
const timelineCollapsed = ref(true);
const showFinishedControls = ref(false);
const selectedVideoName = ref("视频 1");
const selectedStyleName = ref("视频轨道 V1");
const subtitleText = ref("这是一段字幕内容");
const timelinePulse = ref(false);

const draftLibraryVisible = ref(false);
const finishedLibraryVisible = ref(false);
const draftFilter = ref("all");
const editingDraftId = ref("");
const draftEditTitle = ref("");

const exportModalVisible = ref(false);
const exportState = ref("confirm");
const exportProgress = ref(0);
const exportStatus = ref("正在渲染视频文件...");
let exportInterval = null;
let timelineMoveHandler = null;
let timelineUpHandler = null;

const mainVideoRef = ref(null);
const modalVideoRef = ref(null);
const playerStageRef = ref(null);
const playerWrapperRef = ref(null);
const timelineTrackRef = ref(null);
const timelineDragging = ref(false);
const playerPaused = ref(true);
const playerMuted = ref(false);
const playerSpeed = ref(1);
const playerProgress = ref(0);
const playerTimeLabel = ref("00:00 / 00:00");
const modalPaused = ref(true);

const timeline = reactive({
  totalDuration: 50,
  selectedDuration: 20,
  startTime: 0,
});

const stylePresets = ref([
  { id: 1, name: "极简留白", count: 5, imported: false, videos: [] },
  { id: 2, name: "胶片质感", count: 4, imported: false, videos: [] },
  { id: 3, name: "动态转场", count: 2, imported: false, videos: [] },
]);

const draftProjects = ref([
  { id: "draft-wedding-highlight", title: "婚礼亮点快剪", status: "editing", duration: "00:32", time: "保存于 15:24", image: weddingImage },
  { id: "draft-travel-vlog", title: "旅行 Vlog 工程", status: "exported", duration: "00:18", time: "导出于 21:08", image: travelImage },
  { id: "draft-product-launch", title: "新品发布节奏版", status: "editing", duration: "01:06", time: "保存于 10:42", image: weddingImage },
  { id: "draft-city-night", title: "城市夜景混剪", status: "exported", duration: "00:47", time: "导出于 昨天 18:20", image: travelImage },
  { id: "draft-food-review", title: "探店美食短片", status: "editing", duration: "00:26", time: "保存于 昨天 22:12", image: weddingImage },
  { id: "draft-brand-story", title: "品牌故事成片", status: "exported", duration: "02:10", time: "导出于 周一 09:35", image: travelImage },
]);

const sidebarContextLabel = computed(() => {
  if (currentViewState.value === "finished") return "成片素材";
  if (currentViewState.value === "segments") return "模板详情";
  if (currentViewState.value === "import") return "风格库";
  return "";
});
const sidebarTitle = computed(() => {
  if (currentViewState.value === "finished") return "已导入视频";
  if (currentViewState.value === "segments") return activeTemplateName.value;
  if (currentViewState.value === "import") return "素材导入";
  return "工作台";
});
const visibleSegments = computed(() => templateSegments[activeTemplateName.value] || templateSegments["婚礼亮点"]);
const visibleDraftProjects = computed(() =>
  draftFilter.value === "all"
    ? draftProjects.value
    : draftProjects.value.filter((project) => project.status === draftFilter.value),
);
const timelineWidthPercent = computed(() => `${(timeline.selectedDuration / timeline.totalDuration) * 100}%`);
const timelineLeftPercent = computed(() => `${(timeline.startTime / timeline.totalDuration) * 100}%`);
const timelineRangeLabel = computed(() => {
  const endTime = timeline.startTime + timeline.selectedDuration;
  return `${timeline.startTime.toFixed(1)}s - ${endTime.toFixed(1)}s`;
});
const selectedClipTitle = computed(() => `${selectedVideoName.value} - 主体内容`);

function statusMeta(status) {
  return status === "exported"
    ? { label: "已导出", className: "bg-surface-container-high text-on-surface", detail: "已完成导出" }
    : { label: "编辑中", className: "bg-electric-blue text-white", detail: "工程继续编辑" };
}

function selectCategory(index) {
  activeCategory.value = index;
  showFinishedControls.value = false;
  currentViewState.value = "subtopics";
  mainMode.value = "grid";
  timelineCollapsed.value = true;
  previewModalVisible.value = false;
  sidebarHidden.value = false;
  resetModalPreviewVideo();
  resetMainPlayer();
}

function toggleSidebar(show) {
  sidebarHidden.value = typeof show === "boolean" ? !show : !sidebarHidden.value;
  schedulePlayerResize();
}

function openPreview(title, subtitle) {
  showFinishedControls.value = false;
  activeTemplateName.value = title;
  previewTitle.value = title;
  previewSubtitle.value = subtitle;
  currentViewState.value = "segments";
  previewModalVisible.value = true;
  sidebarHidden.value = false;
  resetModalPreviewVideo();
}

function hidePreviewModal() {
  previewModalVisible.value = false;
  resetModalPreviewVideo();
  if (currentViewState.value === "segments") {
    currentViewState.value = "subtopics";
  }
}

function confirmSelection() {
  previewModalVisible.value = false;
  startEditing();
}

function startEditing() {
  currentViewState.value = "import";
  mainMode.value = "player";
  timelineCollapsed.value = false;
  showFinishedControls.value = false;
  nextTick(() => {
    selectVideoForTimeline("视频 1", "视频轨道 V1");
    schedulePlayerResize();
  });
}

function handleSidebarBack() {
  if (currentViewState.value === "import") {
    showFinishedControls.value = false;
    currentViewState.value = "segments";
    mainMode.value = "grid";
    timelineCollapsed.value = true;
    previewModalVisible.value = true;
  } else if (currentViewState.value === "segments") {
    currentViewState.value = "subtopics";
  }
}

function oneClickImportStyle(style) {
  if (style.imported) return;
  style.videos = [...sourcePool].sort(() => Math.random() - 0.5).slice(0, style.count);
  style.imported = true;
  if (style.videos[0]) {
    selectVideoForTimeline(style.videos[0].name, style.name);
  }
}

function selectVideoForTimeline(videoName, styleName = "") {
  selectedVideoName.value = videoName;
  if (styleName) {
    selectedStyleName.value = styleName;
  }
  timelinePulse.value = true;
  window.setTimeout(() => {
    timelinePulse.value = false;
  }, 160);
}

function toggleTimelineContainer() {
  timelineCollapsed.value = !timelineCollapsed.value;
  schedulePlayerResize();
}

function clampTimelineStart(startTime) {
  const maxStart = Math.max(0, timeline.totalDuration - timeline.selectedDuration);
  return Math.min(Math.max(startTime, 0), maxStart);
}

function startTimelineDrag(event) {
  const track = timelineTrackRef.value;
  if (!track) return;

  const selectionRect = event.currentTarget.getBoundingClientRect();
  const offset = event.clientX - selectionRect.left;
  event.currentTarget.setPointerCapture?.(event.pointerId);

  if (timelineMoveHandler) {
    window.removeEventListener("pointermove", timelineMoveHandler);
  }
  if (timelineUpHandler) {
    window.removeEventListener("pointerup", timelineUpHandler);
  }

  timelineMoveHandler = (moveEvent) => {
    const rect = track.getBoundingClientRect();
    const rawLeft = moveEvent.clientX - rect.left - offset;
    const maxLeft = rect.width * ((timeline.totalDuration - timeline.selectedDuration) / timeline.totalDuration);
    const clampedLeft = Math.min(Math.max(rawLeft, 0), Math.max(0, maxLeft));
    timeline.startTime = clampTimelineStart((clampedLeft / rect.width) * timeline.totalDuration);
  };
  timelineUpHandler = () => {
    timelineDragging.value = false;
    window.removeEventListener("pointermove", timelineMoveHandler);
    window.removeEventListener("pointerup", timelineUpHandler);
    timelineMoveHandler = null;
    timelineUpHandler = null;
  };

  timelineDragging.value = true;
  window.addEventListener("pointermove", timelineMoveHandler);
  window.addEventListener("pointerup", timelineUpHandler);
}

function toggleDraftLibrary() {
  finishedLibraryVisible.value = false;
  draftLibraryVisible.value = !draftLibraryVisible.value;
  if (draftLibraryVisible.value) {
    draftFilter.value = "all";
  }
}

function startDraftTitleEdit(project) {
  editingDraftId.value = project.id;
  draftEditTitle.value = project.title;
}

function saveDraftTitle(project) {
  const nextTitle = draftEditTitle.value.trim();
  if (nextTitle) {
    project.title = nextTitle;
  }
  editingDraftId.value = "";
}

function openDraftProject(projectId) {
  const project = draftProjects.value.find((item) => item.id === projectId);
  const displayName = project?.title || projectId;
  draftLibraryVisible.value = false;
  openPlayerFromLibrary(displayName);
}

function showFinishedLibrary() {
  draftLibraryVisible.value = false;
  finishedLibraryVisible.value = true;
  sidebarHidden.value = true;
}

function hideFinishedLibrary() {
  finishedLibraryVisible.value = false;
}

function openFinishedVideo(videoId) {
  const video = finishedVideos.find((item) => item.id === videoId);
  finishedLibraryVisible.value = false;
  openPlayerFromLibrary(video?.displayName || videoId);
}

function openPlayerFromLibrary(displayName) {
  sidebarHidden.value = false;
  mainMode.value = "player";
  timelineCollapsed.value = false;
  currentViewState.value = "finished";
  activeTemplateName.value = displayName;
  showFinishedControls.value = true;
  selectVideoForTimeline(displayName, "主线叙事");
  resetMainPlayer();
  nextTick(schedulePlayerResize);
}

function resetExportProgress() {
  exportProgress.value = 0;
  exportStatus.value = "正在渲染视频文件...";
}

function showExportConfirmation() {
  if (exportInterval) clearInterval(exportInterval);
  resetExportProgress();
  exportState.value = "confirm";
  exportModalVisible.value = true;
}

function closeExportModal() {
  exportModalVisible.value = false;
  if (exportInterval) {
    clearInterval(exportInterval);
    exportInterval = null;
  }
  resetExportProgress();
}

function startExportProgress() {
  exportState.value = "progress";
  resetExportProgress();
  exportInterval = window.setInterval(() => {
    exportProgress.value = Math.min(100, exportProgress.value + Math.floor(Math.random() * 8) + 2);
    if (exportProgress.value >= 100) {
      exportStatus.value = "导出完成！";
      clearInterval(exportInterval);
      exportInterval = null;
    } else if (exportProgress.value > 80) {
      exportStatus.value = "正在封装容器...";
    } else if (exportProgress.value > 40) {
      exportStatus.value = "正在合成音频轨道...";
    }
  }, 200);
}

function formatPlayerTime(value) {
  if (!Number.isFinite(value)) return "00:00";
  const minutes = Math.floor(value / 60).toString().padStart(2, "0");
  const seconds = Math.floor(value % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function updatePlayerControls() {
  const video = mainVideoRef.value;
  if (!video) return;

  const duration = video.duration || 0;
  playerProgress.value = duration ? Math.max(0, Math.min(100, (video.currentTime / duration) * 100)) : 0;
  playerTimeLabel.value = `${formatPlayerTime(video.currentTime)} / ${formatPlayerTime(duration)}`;
  playerPaused.value = video.paused;
  playerMuted.value = video.muted || video.volume === 0;
  playerSpeed.value = video.playbackRate;
}

function resetMainPlayer() {
  const video = mainVideoRef.value;
  if (!video) return;
  video.pause();
  video.currentTime = 0;
  updatePlayerControls();
}

function togglePlayerPlayback() {
  const video = mainVideoRef.value;
  if (!video) return;
  if (video.paused) {
    video.play().catch(updatePlayerControls);
  } else {
    video.pause();
  }
  updatePlayerControls();
}

function seekPlayerBy(seconds) {
  const video = mainVideoRef.value;
  if (!video) return;
  const duration = video.duration || 0;
  video.currentTime = Math.max(0, Math.min(duration, video.currentTime + seconds));
  updatePlayerControls();
}

function seekPlayerTo(event) {
  const video = mainVideoRef.value;
  if (!video || !video.duration) return;
  const rect = event.currentTarget.getBoundingClientRect();
  const ratio = (event.clientX - rect.left) / rect.width;
  video.currentTime = Math.max(0, Math.min(video.duration, ratio * video.duration));
  updatePlayerControls();
}

function cyclePlayerSpeed() {
  const video = mainVideoRef.value;
  if (!video) return;
  const speeds = [0.5, 1, 1.25, 1.5, 2];
  const currentIndex = speeds.indexOf(video.playbackRate);
  video.playbackRate = speeds[(currentIndex + 1) % speeds.length];
  updatePlayerControls();
}

function togglePlayerMute() {
  const video = mainVideoRef.value;
  if (!video) return;
  video.muted = !video.muted;
  updatePlayerControls();
}

function resetModalPreviewVideo() {
  const video = modalVideoRef.value;
  if (!video) return;
  video.pause();
  video.currentTime = 0;
  modalPaused.value = true;
}

function toggleModalPreviewPlayback() {
  const video = modalVideoRef.value;
  if (!video) return;
  if (video.paused) {
    video.play().catch(() => {
      modalPaused.value = true;
    });
  } else {
    video.pause();
  }
  modalPaused.value = video.paused;
}

function resizePlayerToStage() {
  const stage = playerStageRef.value;
  const wrapper = playerWrapperRef.value;
  if (!stage || !wrapper || mainMode.value !== "player") return;

  const rect = stage.getBoundingClientRect();
  const availableWidth = Math.max(320, rect.width - 64);
  const availableHeight = Math.max(180, rect.height - 64);
  const widthFromHeight = availableHeight * (16 / 9);
  const targetWidth = Math.min(availableWidth, widthFromHeight);
  wrapper.style.width = `${targetWidth}px`;
  wrapper.style.height = `${targetWidth * (9 / 16)}px`;
}

function schedulePlayerResize() {
  requestAnimationFrame(resizePlayerToStage);
}

onMounted(() => {
  document.title = "灵剪 · QuickCut Pro - 视频快速剪辑软件";
  document.documentElement.classList.add("dark");
  window.addEventListener("resize", schedulePlayerResize);
  nextTick(() => {
    updatePlayerControls();
    schedulePlayerResize();
  });
});

onBeforeUnmount(() => {
  document.documentElement.classList.remove("dark");
  window.removeEventListener("resize", schedulePlayerResize);
  if (exportInterval) {
    clearInterval(exportInterval);
  }
  if (timelineMoveHandler) {
    window.removeEventListener("pointermove", timelineMoveHandler);
  }
  if (timelineUpHandler) {
    window.removeEventListener("pointerup", timelineUpHandler);
  }
});
</script>

<template>
  <div class="workspace-page bg-background text-on-surface font-body-md text-body-md selection:bg-primary/30">
    <header class="fixed top-0 left-0 right-0 z-[300] flex flex-col bg-surface-container/80 backdrop-blur-2xl border-b border-primary/20">
      <div class="flex items-center px-6 gap-4 overflow-visible no-scrollbar border-b-2 border-white/10 h-16">
        <div class="flex items-center gap-4 shrink-0">
          <img alt="艾咔" class="h-10 w-auto object-contain" :src="logoImage" />
          <h1 class="font-display text-[26px] font-bold tracking-tight text-on-surface whitespace-nowrap">
            艾咔· <span class="text-electric-blue font-black">专业版</span>
          </h1>
        </div>
        <div class="flex items-center gap-1 rounded-full border p-0.5 shrink-0 bg-surface-container-lowest border-outline-variant/30 shadow-inner ml-4">
          <div class="flex items-center gap-1.5 px-3 py-1 bg-surface-container-high border border-electric-blue/40 rounded-full shadow-lg shadow-electric-blue/10">
            <span class="text-[13px] text-electric-blue font-bold whitespace-nowrap">企业: 星辰传媒</span>
          </div>
          <div class="flex items-center gap-1.5 px-3 py-1 rounded-full">
            <span class="text-[13px] text-on-surface-variant font-medium whitespace-nowrap">剪辑师_01</span>
          </div>
        </div>
        <div class="flex-1"></div>
        <button class="h-9 w-24 text-on-surface-variant hover:text-electric-blue shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 shadow-sm rounded-lg transition-all active:scale-95 hover:bg-surface-container-high border border-outline-variant/20" @click="toggleDraftLibrary">
          <span class="text-[13px] font-bold whitespace-nowrap">工程库</span>
        </button>
        <button class="h-9 w-24 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant hover:text-electric-blue rounded-lg font-bold shadow-sm hover:bg-surface-container-high active:scale-95 transition-all shrink-0 border border-outline-variant/20" @click="showExportConfirmation">
          <span class="text-[13px] uppercase tracking-wide whitespace-nowrap">导出</span>
        </button>
        <div class="relative group z-[130]">
          <button class="h-9 w-24 shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant shadow-sm hover:bg-surface-container-high hover:text-electric-blue focus:bg-electric-blue focus:text-white rounded-lg transition-all focus:outline-none active:scale-95 border border-outline-variant/20">
            <span class="text-[13px] font-bold">个人中心</span>
          </button>
          <div class="absolute top-full right-0 mt-2 w-40 bg-surface-container-highest border border-outline-variant/30 rounded-lg shadow-2xl opacity-0 translate-y-2 pointer-events-none group-focus-within:opacity-100 group-focus-within:translate-y-0 group-focus-within:pointer-events-auto transition-all duration-200 ease-out z-[110] py-1">
            <a class="flex items-center gap-3 px-4 py-2.5 text-[12px] text-on-surface-variant hover:bg-primary-container/10 hover:text-white transition-colors" href="#"><span>我的咔币</span></a>
            <div class="mx-2 my-1 border-t border-outline-variant/30"></div>
            <a class="flex items-center gap-3 px-4 py-2.5 text-[12px] text-on-surface-variant hover:bg-primary-container/10 hover:text-white transition-colors" href="#"><span>帮助</span></a>
            <div class="mx-2 my-1 border-t border-outline-variant/30"></div>
            <a class="flex items-center gap-3 px-4 py-2.5 text-[12px] text-error hover:bg-error-container/20 transition-colors" href="#" @click.prevent="emit('logout')"><span>退出登录</span></a>
          </div>
        </div>
      </div>

      <div class="h-14 category-toolbar border-b border-primary/10 flex items-center px-6 gap-4 overflow-x-auto no-scrollbar">
        <span class="text-on-surface-variant font-label-caps text-[13px] uppercase shrink-0 whitespace-nowrap">当前模板主题</span>
        <div class="category-container flex items-center gap-2 shrink-0">
          <button
            v-for="(category, index) in categories"
            :key="category"
            class="category-btn flex items-center gap-2 px-4 py-1.5 rounded-full border text-[12px] font-bold whitespace-nowrap transition-all duration-300"
            :class="activeCategory === index ? 'bg-electric-blue text-white border-white active-glow' : 'border-outline-variant/30 bg-white/5 text-on-surface-variant hover:border-electric-blue/50 hover:text-electric-blue'"
            @click="selectCategory(index)"
          >
            <span>{{ category }}</span>
          </button>
        </div>
        <div class="flex-1"></div>
        <button
          class="category-btn flex items-center gap-2 px-4 py-1.5 rounded-full border text-[12px] font-bold whitespace-nowrap transition-all duration-300 shrink-0 border-outline-variant/30 bg-white/5 text-on-surface-variant hover:border-electric-blue/50 hover:text-electric-blue"
          type="button"
        >
          <span>收藏夹</span>
        </button>
        <div class="relative max-w-[200px] shrink-0">
          <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[16px] text-on-surface-variant/50">search</span>
          <input class="w-full bg-surface-container-lowest/50 border border-outline-variant/30 rounded-full pl-9 pr-4 py-1.5 text-[11px] focus:border-electric-blue/50 focus:bg-surface-container-lowest outline-none transition-all text-on-surface placeholder:text-on-surface-variant/40" placeholder="搜索主题..." type="text" />
        </div>
      </div>
    </header>

    <main class="h-[calc(100vh-120px)] flex flex-col mt-[120px]">
      <div class="flex-1 flex overflow-hidden relative">
        <aside class="w-72 flex flex-col z-[150] h-full shrink-0 relative shadow-2xl" :class="{ 'hidden-sidebar': sidebarHidden }">
          <div class="p-4 border-b border-white/5 flex justify-between items-center bg-white/5">
            <h2 class="font-header-section text-body-md font-bold text-on-surface flex items-center gap-2 whitespace-nowrap flex-1 overflow-hidden">
              <div class="flex flex-col overflow-hidden">
                <span class="text-[12px] text-on-surface-variant/60 uppercase tracking-[0.18em]">{{ sidebarContextLabel }}</span>
                <span class="text-[17px] tracking-[0.18em] truncate">{{ sidebarTitle }}</span>
              </div>
              <div class="flex-1"></div>
              <button v-if="currentViewState === 'segments' || currentViewState === 'import'" class="flex items-center gap-1 px-2.5 py-1.5 rounded-md bg-electric-blue text-[12px] text-white font-bold shrink-0 hover:brightness-110 active:scale-95 transition-all" @click="handleSidebarBack">
                <span class="text-[11px]">返回</span>
                <span class="material-symbols-outlined text-[18px]">arrow_forward</span>
              </button>
            </h2>
          </div>
          <div class="flex-1 overflow-y-auto custom-scrollbar flex flex-col">
            <div v-if="currentViewState === 'subtopics'" class="p-4 space-y-4">
              <h3 class="text-[13px] font-bold text-on-surface-variant uppercase tracking-widest whitespace-nowrap mb-2">主题名称</h3>
              <div class="grid grid-cols-1 gap-2">
                <button v-for="topic in subtopics" :key="topic.title" class="px-2 py-2 rounded bg-surface-container border border-outline-variant text-on-surface-variant hover:border-electric-blue/40 hover:bg-surface-container-high transition-all flex flex-col gap-1 text-left group overflow-hidden" @click="openPreview(topic.title, topic.subtitle)">
                  <div class="flex items-center gap-2">
                    <div class="text-[10px] font-bold text-on-surface truncate">{{ topic.title }} ({{ topic.count }})</div>
                  </div>
                  <div class="flex gap-2">
                    <span class="text-[9px] opacity-60">素材: {{ topic.material }}</span>
                    <span class="text-[9px] opacity-60">时长: {{ topic.duration }}</span>
                  </div>
                </button>
              </div>
            </div>
            <div v-else-if="currentViewState === 'segments'" class="p-4 space-y-4 flex flex-col h-full">
              <div class="flex items-center justify-between mb-2 shrink-0">
                <h3 class="text-[10px] font-bold text-on-surface-variant uppercase tracking-widest">模板片段详情</h3>
              </div>
              <div class="flex-1 overflow-y-auto custom-scrollbar space-y-3 pt-1">
                <div v-for="segment in visibleSegments" :key="segment.name" class="segment-card p-3 rounded bg-surface-container-high/50 border border-white/5 hover:border-electric-blue/40 transition-all transform hover:-translate-y-0.5 cursor-pointer shadow-sm">
                  <div class="text-[12px] font-bold text-on-surface">{{ segment.name }}</div>
                  <div class="flex flex-col gap-1 mt-1.5">
                    <div class="flex justify-between text-[10px]">
                      <span class="text-on-surface-variant">主题标签:</span>
                      <span class="text-electric-blue font-bold">{{ segment.shot }}</span>
                    </div>
                    <div class="flex justify-between text-[10px]">
                      <span class="text-on-surface-variant">所需素材:</span>
                      <span class="text-electric-blue font-bold">{{ segment.count }} 个视频</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="p-4 space-y-4 flex flex-col h-full">
              <div class="flex-1 overflow-y-auto custom-scrollbar space-y-3 p-1">
                <template v-if="currentViewState === 'finished'">
                  <div v-for="group in finishedGroups" :key="group.name" class="videoList rounded-lg bg-surface-container-low border border-white/5 overflow-hidden mb-3 shadow-sm">
                    <div class="flex items-center justify-between p-3 bg-white/5">
                      <div class="flex items-center gap-2 min-w-0">
                        <span class="material-symbols-outlined text-primary text-[18px] shrink-0">folder_open</span>
                        <span class="text-[13px] font-bold text-white truncate">风格: {{ group.name }}</span>
                      </div>
                      <span class="text-[10px] font-bold text-primary shrink-0">{{ group.videos.length }} 个视频</span>
                    </div>
                    <div class="p-3 space-y-2 border-t border-outline-variant">
                      <div v-for="video in group.videos" :key="video.name" class="flex items-center justify-between p-2 rounded bg-surface-container-lowest/50 border border-white/5 hover:border-electric-blue/40 transition-all cursor-pointer group" :class="{ 'is-selected': selectedVideoName === video.name }" @click="selectVideoForTimeline(video.name, group.name)">
                        <div class="flex items-center gap-2 min-w-0">
                          <div class="w-12 aspect-video rounded bg-black/70 border border-white/10 flex items-center justify-center shrink-0">
                            <span class="material-symbols-outlined text-primary text-[18px]">movie</span>
                          </div>
                          <div class="min-w-0">
                            <div class="text-[12px] text-white truncate">{{ video.name }}</div>
                            <div class="text-[10px] text-on-surface-variant font-code-data">{{ video.time }}</div>
                          </div>
                        </div>
                        <button class="flex items-center gap-1 px-2 py-1 bg-white/5 hover:bg-white/10 rounded text-[11px] text-white/80 transition-colors shrink-0" @click.stop>
                          <span class="material-symbols-outlined text-[15px]">sync</span>
                          替换
                        </button>
                      </div>
                    </div>
                  </div>
                </template>
                <template v-else>
                  <div v-for="style in stylePresets" :key="style.id" class="videoList rounded-lg bg-surface-container-low border border-white/5 overflow-hidden mb-3 shadow-sm">
                    <div class="flex items-center justify-between p-3 bg-white/5">
                      <div class="flex items-center gap-2">
                        <span class="text-[13px] font-bold text-white">风格: {{ style.name }}</span>
                      </div>
                      <button class="px-3 py-1.5 bg-electric-blue text-white text-[11px] font-bold rounded-md flex items-center gap-1 shadow-lg shadow-electric-blue/10 disabled:opacity-60 disabled:cursor-default" :disabled="style.imported" @click="oneClickImportStyle(style)">
                        {{ style.imported ? `已导入 (${style.videos.length})` : `一键导入 (${style.count})` }}
                      </button>
                    </div>
                    <div v-if="style.imported" class="p-3 space-y-2 border-t border-outline-variant">
                      <div v-for="video in style.videos" :key="`${style.id}-${video.name}`" class="flex items-center justify-between p-2 rounded bg-surface-container-lowest/50 border border-white/5 hover:border-electric-blue/40 transition-all cursor-pointer group" :class="{ 'is-selected': selectedVideoName === video.name }" @click="selectVideoForTimeline(video.name, style.name)">
                        <div class="flex items-center gap-2 min-w-0">
                          <span class="material-symbols-outlined text-primary text-[18px] shrink-0">smart_display</span>
                          <div class="min-w-0">
                            <div class="text-[12px] text-white truncate">{{ video.name }}</div>
                            <div class="text-[10px] text-on-surface-variant">{{ video.tag }} · {{ video.duration }}</div>
                          </div>
                        </div>
                        <div class="px-2 py-1 bg-electric-blue/10 rounded text-[10px] text-electric-blue font-bold shrink-0">替换</div>
                      </div>
                    </div>
                  </div>
                </template>
              </div>
            </div>
          </div>
        </aside>

        <section class="flex-1 bg-surface-dim flex flex-col h-full overflow-hidden relative">
          <button class="absolute top-1/2 left-0 -translate-y-1/2 z-[80] bg-surface-container-high border border-outline-variant text-white w-6 h-12 rounded-r-full flex items-center justify-center shadow-lg hover:bg-surface-container-highest transition-all duration-300 group" @click="toggleSidebar()">
            <span class="material-symbols-outlined text-[18px] group-hover:scale-110">{{ sidebarHidden ? "chevron_right" : "chevron_left" }}</span>
          </button>
          <div class="absolute inset-0 z-[190] bg-black/60 backdrop-blur-[2px] modal-fade-in" :class="{ hidden: !previewModalVisible }" @click="hidePreviewModal"></div>

          <div v-show="mainMode === 'grid'" class="w-full h-full flex flex-col relative z-0">
            <div class="p-6 shrink-0">
              <h2 class="text-[18px] font-black text-on-surface flex items-center gap-2">为您推荐</h2>
            </div>
            <div class="flex-1 overflow-y-auto custom-scrollbar p-6 pt-0">
              <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
                <div v-for="card in recommendationCards" :key="card.title" class="group relative aspect-[16/9] bg-surface-container-high rounded-xl overflow-hidden border border-white/5 hover:border-electric-blue/60 transition-all cursor-pointer shadow-lg hover:shadow-electric-blue/10" @click="openPreview(card.title, card.subtitle)">
                  <img :alt="card.title" class="w-full h-full object-cover opacity-70 group-hover:scale-105 transition-transform duration-500" :src="card.image" />
                  <div class="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent flex flex-col justify-end p-4">
                    <div class="text-[14px] font-black text-white">{{ card.title }}</div>
                    <div class="text-[10px] text-white/60">{{ card.meta }}</div>
                  </div>
                  <div v-if="card.badge" class="absolute top-2 right-2 bg-electric-blue text-white text-[9px] font-black px-2 py-0.5 rounded">{{ card.badge }}</div>
                </div>
              </div>
            </div>
          </div>

          <div v-show="mainMode === 'player'" class="w-full flex-1 min-h-0 flex flex-col">
            <div ref="playerStageRef" class="flex-1 bg-black/90 flex flex-col items-center justify-center relative overflow-hidden p-8">
              <div ref="playerWrapperRef" class="playerWrapper relative rounded-xl overflow-hidden shadow-2xl border border-white/10">
                <video ref="mainVideoRef" class="w-full h-full object-cover bg-black" playsinline preload="metadata" :src="videoSource" @loadedmetadata="updatePlayerControls" @timeupdate="updatePlayerControls" @play="updatePlayerControls" @pause="updatePlayerControls" @volumechange="updatePlayerControls" @ratechange="updatePlayerControls"></video>
                <img alt="模板预览" class="hidden" :src="previewImage" />
                <div class="absolute inset-0 flex items-center justify-center bg-black/20 group">
                  <button class="w-20 h-20 bg-white/20 backdrop-blur-md rounded-full flex items-center justify-center border border-white/40 hover:scale-110 transition-transform" @click="togglePlayerPlayback">
                    <span class="material-symbols-outlined text-white text-[48px]" style="font-variation-settings: 'FILL' 1">{{ playerPaused ? "play_arrow" : "pause" }}</span>
                  </button>
                </div>
                <div class="finished-playback-controls absolute inset-x-0 bottom-0 px-4 pt-10 pb-4" :class="{ hidden: !showFinishedControls }">
                  <div class="finished-progress-track mb-3 cursor-pointer" @click="seekPlayerTo">
                    <div class="finished-progress-fill" :style="{ width: `${playerProgress}%` }"></div>
                  </div>
                  <div class="flex items-center gap-3 text-white">
                    <button class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors" @click="seekPlayerBy(-10)"><span class="material-symbols-outlined text-[18px]">replay_10</span></button>
                    <button class="w-9 h-9 rounded-full bg-electric-blue hover:brightness-110 flex items-center justify-center shadow-lg shadow-electric-blue/20 transition-all" @click="togglePlayerPlayback"><span class="material-symbols-outlined text-[22px]" style="font-variation-settings: 'FILL' 1">{{ playerPaused ? "play_arrow" : "pause" }}</span></button>
                    <button class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors" @click="seekPlayerBy(10)"><span class="material-symbols-outlined text-[18px]">forward_10</span></button>
                    <div class="text-[11px] font-code-data text-white/80">{{ playerTimeLabel }}</div>
                    <div class="flex-1"></div>
                    <button class="px-2.5 h-8 rounded bg-white/10 hover:bg-white/20 text-[11px] font-bold transition-colors" @click="cyclePlayerSpeed">{{ playerSpeed.toFixed(1) }}x</button>
                    <button class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors" @click="togglePlayerMute"><span class="material-symbols-outlined text-[18px]">{{ playerMuted ? "volume_off" : "volume_up" }}</span></button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="timeline-dock relative shrink-0" :class="{ 'timeline-collapsed': timelineCollapsed }">
            <button class="absolute left-1/2 -translate-x-1/2 z-[70] h-7 w-12 rounded-t-full bg-surface-container-high border border-outline-variant text-white shadow-lg flex items-center justify-center transition-all duration-200" :class="timelineCollapsed ? 'top-[-28px]' : 'top-0'" @click="toggleTimelineContainer">
              <span class="material-symbols-outlined text-[20px]">{{ timelineCollapsed ? "keyboard_arrow_up" : "keyboard_arrow_down" }}</span>
              <span class="sr-only">展开时间线</span>
            </button>
            <div class="timeline-panel bg-surface-container-lowest border-t border-outline-variant flex flex-col">
              <div class="track-area custom-scrollbar overflow-x-auto" style="background: #030d25; padding: 26px 12px 60px">
                <div class="track w-full" style="background: #07122a; border: 1px solid rgba(74, 142, 255, 0.15); border-radius: 4px; margin-bottom: 8px">
                  <div class="track-title flex items-center gap-2" style="font-size: 10px; font-weight: 700; color: rgba(217, 226, 255, 0.5); border-bottom: 1px solid rgba(74, 142, 255, 0.15); background: rgba(16, 27, 51, 0.8)">
                    {{ selectedStyleName }}
                  </div>
                  <div class="timeline-ruler" aria-label="时间刻度">
                    <div class="timeline-ruler-major"></div>
                    <div class="timeline-ruler-labels"><span>0s</span><span>10s</span><span>20s</span><span>30s</span><span>40s</span><span>50s</span></div>
                  </div>
                  <div class="clips-row relative h-16">
                    <div ref="timelineTrackRef" class="duration-track">
                      <div class="duration-selection bg-electric-blue/20 border-2 border-electric-blue rounded-md flex items-center px-3 shadow-[0_0_15px_rgba(74,142,255,0.25)]" :class="{ 'is-dragging': timelineDragging }" :style="{ left: timelineLeftPercent, width: timelineWidthPercent, transform: `translateY(-50%) scale(${timelinePulse && !timelineDragging ? 1.02 : 1})`, boxShadow: timelinePulse && !timelineDragging ? '0 0 20px rgba(74,142,255,0.45)' : '0 0 15px rgba(74,142,255,0.25)' }" @pointerdown="startTimelineDrag">
                        <div class="flex flex-col justify-between h-full py-1 px-2 w-full min-w-0">
                          <div class="flex items-center gap-1.5 min-w-0"><span class="text-[12px] font-bold text-white tracking-wide truncate">{{ selectedClipTitle }}</span></div>
                          <div class="flex items-baseline gap-2">
                            <span class="text-[10px] font-bold text-primary">{{ timeline.selectedDuration }}s</span>
                            <div class="flex-1"></div>
                            <span class="text-[10px] font-medium text-white/90 font-code-data tracking-tighter">{{ timelineRangeLabel }}</span>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                <div class="w-full flex items-center gap-3 px-4 py-3 bg-surface-container-low border border-outline-variant rounded">
                  <label class="text-[12px] font-bold text-on-surface-variant shrink-0 whitespace-nowrap" for="subtitle-edit-input">标题编辑</label>
                  <input id="subtitle-edit-input" v-model="subtitleText" class="w-[360px] max-w-[42vw] h-9 bg-surface-container-lowest/50 border border-outline-variant/30 rounded px-3 text-[12px] text-on-surface placeholder:text-on-surface-variant/50 focus:border-electric-blue outline-none transition-colors" placeholder="输入当前片段字幕内容" type="text" />
                  <button class="h-9 px-4 bg-electric-blue text-white rounded text-[12px] font-bold shadow-lg shadow-electric-blue/20 hover:brightness-110 active:scale-95 transition-all shrink-0">应用更改</button>
                </div>
              </div>
            </div>
          </div>

          <div class="absolute inset-0 z-[200] flex items-center justify-center" :class="{ hidden: !previewModalVisible }">
            <div class="relative w-full max-w-4xl mx-4 bg-surface-container rounded-2xl overflow-hidden shadow-[0_30px_90px_rgba(0,0,0,0.8)] border border-electric-blue/20 modal-pop-in">
              <div class="absolute top-4 right-4 z-10 flex items-center gap-3">
                <button class="p-2 bg-black/40 backdrop-blur-md rounded-full text-white/80 hover:text-white transition-colors group"><span class="material-symbols-outlined text-[24px] group-hover:scale-110">star</span></button>
                <button class="p-2 bg-black/40 backdrop-blur-md rounded-full text-white/80 hover:text-white transition-colors group" @click="hidePreviewModal"><span class="material-symbols-outlined text-[24px] group-hover:scale-110">close</span></button>
              </div>
              <div class="aspect-video bg-black flex items-center justify-center relative group cursor-pointer">
                <video ref="modalVideoRef" class="w-full h-full object-cover" playsinline preload="metadata" :src="videoSource" @play="modalPaused = false" @pause="modalPaused = true" @ended="modalPaused = true"></video>
                <img alt="预览" class="hidden" :src="previewImage" />
                <div class="absolute inset-0 flex items-center justify-center bg-black/20">
                  <button class="w-20 h-20 bg-white/5 backdrop-blur-3xl rounded-full flex items-center justify-center border border-white/10 hover:scale-110 transition-transform shadow-2xl" @click="toggleModalPreviewPlayback">
                    <span class="material-symbols-outlined text-white text-[48px]" style="font-variation-settings: 'FILL' 1">{{ modalPaused ? "play_arrow" : "pause" }}</span>
                  </button>
                </div>
              </div>
              <div class="p-6 flex items-center justify-between bg-surface-container-lowest/80 border-t border-white/5">
                <div>
                  <h3 class="text-lg font-black text-on-surface">{{ previewTitle }}</h3>
                  <p class="text-sm text-on-surface-variant">{{ previewSubtitle }}</p>
                </div>
                <div class="flex items-center gap-4">
                  <button class="px-10 py-3 bg-electric-blue text-white rounded-lg font-bold text-[16px] shadow-2xl shadow-electric-blue/30 hover:brightness-110 active:scale-95 transition-all uppercase" @click="confirmSelection">开始编辑</button>
                </div>
              </div>
            </div>
          </div>

          <div class="fixed inset-0 z-[400] flex items-center justify-center" :class="{ hidden: !exportModalVisible }">
            <div class="absolute inset-0 bg-black/60 backdrop-blur-2xl"></div>
            <div v-if="exportState === 'confirm'" class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-8 border border-white/10 shadow-2xl modal-pop-in">
              <div class="flex flex-col items-center text-center space-y-6">
                <div class="w-16 h-16 bg-electric-blue/10 rounded-full flex items-center justify-center"><span class="material-symbols-outlined text-electric-blue text-3xl">payments</span></div>
                <div>
                  <h3 class="text-xl font-black text-white mb-2">确认导出工程</h3>
                  <p class="text-on-surface-variant text-sm">本次导出将扣除 <span class="text-electric-blue font-bold">10 积分</span></p>
                </div>
                <div class="flex flex-col w-full gap-3">
                  <button class="w-full py-3 bg-electric-blue text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95 shadow-lg shadow-electric-blue/20" @click="startExportProgress">确认导出</button>
                  <button class="w-full py-3 bg-transparent text-on-surface-variant font-bold rounded-xl hover:text-white transition-colors" @click="closeExportModal">再想想</button>
                </div>
              </div>
            </div>
            <div v-else class="relative w-full max-w-md bg-surface-container-highest rounded-2xl p-10 border border-white/10 shadow-2xl modal-pop-in">
              <div class="flex flex-col items-center space-y-8">
                <div class="circular-progress" :style="{ '--progress': `${exportProgress}%` }">
                  <div class="absolute inset-0 flex items-center justify-center"><span class="text-2xl font-black text-electric-blue">{{ exportProgress }}%</span></div>
                </div>
                <div class="text-center">
                  <h3 class="text-lg font-bold text-white mb-1">{{ exportStatus }}</h3>
                  <p class="text-xs text-on-surface-variant">请勿关闭当前窗口</p>
                </div>
                <div class="w-full space-y-4">
                  <div class="h-1.5 w-full bg-white/5 rounded-full overflow-hidden"><div class="h-full bg-electric-blue transition-all duration-300 shadow-[0_0_12px_rgba(74,142,255,0.8)]" :style="{ width: `${exportProgress}%` }"></div></div>
                  <button v-if="exportProgress < 100" class="w-full py-2.5 bg-white/5 text-on-surface-variant text-sm font-bold rounded-lg hover:bg-white/10 hover:text-white transition-all" @click="closeExportModal">取消导出</button>
                  <button v-else class="w-full py-3 bg-electric-blue text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95" @click="closeExportModal">完成</button>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>
    </main>

    <div class="absolute top-[120px] left-0 right-0 bottom-0 bg-background flex flex-col z-[210]" :class="{ hidden: !draftLibraryVisible }">
      <div class="p-6 border-b border-white/5 flex items-center justify-between bg-surface-container/80 backdrop-blur-3xl">
        <div class="flex items-center gap-4">
          <button class="flex items-center gap-2 text-on-surface-variant hover:text-electric-blue bg-surface-container-lowest px-3 py-1.5 rounded-lg border border-white/5 transition-all" @click="toggleDraftLibrary"><span class="material-symbols-outlined text-[20px]">arrow_back</span><span class="text-[12px] font-bold">返回</span></button>
          <h2 class="font-header-section text-on-surface flex items-center gap-2">工程库</h2>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-[11px] font-bold text-on-surface-variant">分类</span>
          <div class="relative">
            <select v-model="draftFilter" class="appearance-none h-9 min-w-[112px] bg-surface-container-lowest border border-white/5 rounded-md pl-3 pr-9 text-[12px] font-bold text-on-surface outline-none hover:border-electric-blue/50 focus:border-electric-blue transition-colors shadow-inner">
              <option value="all">全部</option>
              <option value="editing">编辑中</option>
              <option value="exported">已导出</option>
            </select>
            <span class="material-symbols-outlined pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[18px] text-electric-blue">expand_more</span>
          </div>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-6">
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
          <div v-for="project in visibleDraftProjects" :key="project.id" class="group relative aspect-[16/9] bg-surface-container-high rounded-xl overflow-hidden border border-white/5 hover:border-electric-blue/60 transition-all cursor-pointer shadow-lg hover:shadow-electric-blue/10">
            <img alt="draft preview" class="w-full h-full object-cover opacity-70 group-hover:scale-105 transition-transform duration-500 cursor-pointer" :src="project.image" @click="openDraftProject(project.id)" />
            <div class="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent flex flex-col justify-end p-4 pointer-events-none">
              <input v-if="editingDraftId === project.id" v-model="draftEditTitle" class="pointer-events-auto max-w-[180px] rounded bg-surface-container-lowest/80 border border-electric-blue/60 px-1.5 py-0.5 text-[12px] font-bold text-white outline-none" aria-label="编辑工程名称" @click.stop @keydown.enter.prevent="saveDraftTitle(project)" @keydown.esc.prevent="editingDraftId = ''" @blur="saveDraftTitle(project)" />
              <button v-else class="draft-title-btn pointer-events-auto text-left text-[14px] font-black text-white truncate hover:text-electric-blue transition-colors" type="button" title="点击编辑工程名称" @click.stop="startDraftTitleEdit(project)">{{ project.title }}</button>
              <div class="text-[10px] text-white/60">{{ statusMeta(project.status).detail }} · {{ project.duration }} · {{ project.time }}</div>
            </div>
            <div class="absolute top-3 right-3 text-[11px] font-black px-3 py-1 rounded-md shadow-lg" :class="statusMeta(project.status).className">{{ statusMeta(project.status).label }}</div>
          </div>
        </div>
      </div>
    </div>

    <div class="absolute top-[120px] left-0 right-0 bottom-0 bg-background flex flex-col z-[210]" :class="{ hidden: !finishedLibraryVisible }">
      <div class="p-6 border-b border-white/5 flex items-center justify-between bg-surface-container/80 backdrop-blur-3xl">
        <div class="flex items-center gap-4">
          <button class="flex items-center gap-2 text-on-surface-variant hover:text-electric-blue bg-surface-container-lowest px-3 py-1.5 rounded-lg border border-white/5 transition-all" @click="hideFinishedLibrary"><span class="material-symbols-outlined text-[20px]">arrow_back</span><span class="text-[12px] font-bold">返回</span></button>
          <h2 class="font-header-section text-on-surface flex items-center gap-2">成片库</h2>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-6">
        <div class="space-y-4 max-w-5xl mx-auto">
          <div class="flex items-center px-4 py-2 text-[10px] font-bold text-on-surface-variant/50 uppercase tracking-widest border-b border-outline-variant/30">
            <div class="w-48 shrink-0">预览</div>
            <div class="flex-1">成片名称</div>
            <div class="w-24 text-center">时长</div>
            <div class="w-32 text-center">生成时间</div>
            <div class="w-24"></div>
          </div>
          <div v-for="video in finishedVideos" :key="video.id" class="flex items-center gap-6 p-4 rounded-xl bg-surface-container-high/40 border border-white/5 hover:border-electric-blue/40 transition-all group cursor-pointer" @click="openFinishedVideo(video.id)">
            <div class="w-48 aspect-video bg-surface-container-highest rounded-lg overflow-hidden relative border border-outline-variant/30 shrink-0">
              <img alt="finished video preview" class="w-full h-full object-cover opacity-75" :src="video.image" />
              <div class="absolute inset-0 flex items-center justify-center bg-black/30"><span class="material-symbols-outlined text-white/70">play_circle</span></div>
            </div>
            <div class="flex-1 min-w-0">
              <h3 class="text-sm font-bold text-on-surface truncate">{{ video.title }}</h3>
              <p class="text-[11px] text-on-surface-variant/60 mt-1">1080p | H.264 | 已导出</p>
            </div>
            <div class="w-24 text-center text-[12px] font-medium text-on-surface-variant">{{ video.duration }}</div>
            <div class="w-32 text-center text-[11px] text-on-surface-variant">{{ video.date }}</div>
            <div class="w-24 flex justify-end"><button class="flex items-center gap-1.5 px-4 py-2 bg-electric-blue/10 hover:bg-electric-blue text-electric-blue hover:text-white rounded-lg text-[11px] font-bold transition-all">打开</button></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
.workspace-page {
  width: 100%;
  height: 100vh;
  overflow: hidden;
  position: relative;
  background: #07122a;
}

.workspace-page .material-symbols-outlined {
  font-variation-settings:
    "FILL" 0,
    "wght" 400,
    "GRAD" 0,
    "opsz" 24;
  vertical-align: middle;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(74, 142, 255, 0.2);
  border-radius: 10px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(74, 142, 255, 0.4);
}

.track-area {
  flex: 1 1 0;
  height: 100%;
  max-height: 100%;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 12px 12px 56px;
}

.track {
  background: #030d25;
  border-radius: 12px;
  margin-bottom: 20px;
  padding: 6px 0;
}

.track-title {
  font-size: 11px;
  padding: 4px 12px;
  color: rgba(217, 226, 255, 0.5);
}

.clips-row {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  flex-wrap: wrap;
}

.duration-track {
  position: relative;
  width: 100%;
  height: 44px;
  border-radius: 8px;
  border: 1px solid rgba(74, 142, 255, 0.35);
  background: rgba(16, 27, 51, 0.95);
  box-shadow: inset 0 0 0 1px rgba(217, 226, 255, 0.06);
  overflow: hidden;
}

.duration-selection {
  position: absolute;
  top: 50%;
  height: 38px;
  min-width: 80px;
  cursor: grab;
  user-select: none;
  touch-action: none;
  transition: 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.duration-selection.is-dragging {
  cursor: grabbing;
  transition: none !important;
}

.timeline-ruler {
  height: 32px;
  position: relative;
  margin: 0 12px;
  border-bottom: 1px solid rgba(74, 142, 255, 0.2);
  border-right: 1px solid rgba(217, 226, 255, 0.55);
  background: rgba(16, 27, 51, 0.72);
  overflow: hidden;
}

.timeline-ruler::before {
  content: "";
  position: absolute;
  inset: 0 0 auto;
  height: 8px;
  background: repeating-linear-gradient(to right, rgba(217, 226, 255, 0.28) 0, rgba(217, 226, 255, 0.28) 1px, transparent 1px, transparent 2%);
  pointer-events: none;
}

.timeline-ruler-labels {
  position: absolute;
  inset: 0 10px;
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding-top: 5px;
  color: rgba(217, 226, 255, 0.72);
  font-family: "JetBrains Mono", monospace;
  font-size: 10px;
  font-weight: 700;
  pointer-events: none;
}

.timeline-ruler-major {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(to right, rgba(217, 226, 255, 0.55) 0, rgba(217, 226, 255, 0.55) 1px, transparent 1px, transparent 20%);
  pointer-events: none;
}

.finished-playback-controls {
  background: linear-gradient(to top, rgba(3, 13, 37, 0.95), rgba(3, 13, 37, 0.4), transparent);
}

.finished-progress-track {
  height: 4px;
  border-radius: 999px;
  background: rgba(217, 226, 255, 0.15);
  overflow: hidden;
}

.finished-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: #4a8eff;
  box-shadow: 0 0 12px rgba(74, 142, 255, 0.8);
}

.videoList .is-selected {
  border-color: #4a8eff !important;
}

.playerWrapper {
  aspect-ratio: 16 / 9;
  max-width: 100%;
  max-height: 100%;
  background: #000000;
  box-shadow: 0 0 40px rgba(74, 142, 255, 0.1);
}

.timeline-dock {
  height: 12rem;
  overflow: visible;
  transition: height 0.25s ease;
  z-index: 50;
}

.timeline-dock.timeline-collapsed {
  height: 0;
}

.timeline-panel {
  height: 12rem;
  overflow: hidden;
  transition:
    transform 0.25s ease,
    opacity 0.2s ease;
  backdrop-filter: blur(24px);
  background: #07122a;
  border-top: 1px solid rgba(74, 142, 255, 0.2);
}

.timeline-dock.timeline-collapsed .timeline-panel {
  opacity: 0;
  pointer-events: none;
  transform: translateY(100%);
}

aside {
  transition:
    transform 0.3s cubic-bezier(0.4, 0, 0.2, 1),
    margin-right 0.3s cubic-bezier(0.4, 0, 0.2, 1),
    opacity 0.2s ease;
  backdrop-filter: blur(28px);
  background: rgba(21, 31, 55, 0.85);
  border-right: 1px solid rgba(74, 142, 255, 0.15);
}

aside.hidden-sidebar {
  margin-right: -18rem;
  transform: translateX(-100%);
  opacity: 0;
  pointer-events: none;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes popIn {
  from {
    transform: scale(0.95);
    opacity: 0;
  }
  to {
    transform: scale(1);
    opacity: 1;
  }
}

.modal-fade-in {
  animation: fadeIn 0.2s ease-out forwards;
}

.modal-pop-in {
  animation: popIn 0.3s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}

.no-scrollbar::-webkit-scrollbar {
  display: none;
}

.circular-progress {
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

.category-toolbar {
  background: rgba(16, 27, 51, 0.8);
  backdrop-filter: blur(10px);
}

.active-glow {
  box-shadow: 0 0 15px rgba(74, 142, 255, 0.4);
}

.segment-card:hover {
  border-color: rgba(74, 142, 255, 0.55);
  border-top-color: rgba(74, 142, 255, 0.9);
}
</style>

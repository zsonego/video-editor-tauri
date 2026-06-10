<script setup>
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
} from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import {
  createProject,
  deleteProjects,
  getProjectDetail,
  getMyProjects,
  recordProjectExport,
  renameProject,
  updateProject,
} from '../api/project';
import {
  favoriteTemplate,
  getFavoriteTemplates,
  getTemplateCategories,
  getTemplateDetail,
  getTemplates,
} from '../api/template';
import { logoutUser, resetPassword } from '../api/user';
import { systemMessage } from '../utils/message';
import logoImage from '../assets/logo.png';

// 页面对外事件与远程/本地资源配置。
const emit = defineEmits(['logout']);

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '';
const TEMPLATE_DOWNLOAD_BASE_URL =
  import.meta.env.VITE_TEMPLATE_DOWNLOAD_BASE_URL || API_BASE_URL;

// 模板列表、收藏列表及请求状态。
const categories = ref([]);
const recommendationCards = ref([]);
const favoriteTemplates = ref([]);
const templateSearchKeyword = ref('');
const subtopics = ref([]);
const activeTemplateSegments = ref([]);
const templatesLoading = ref(false);
const recommendationsLoading = ref(false);
const favoritesLoading = ref(false);
let templateRequestId = 0;
let recommendationRequestId = 0;
let templateSearchTimer = null;

// 工作区导航、弹窗和当前模板状态。
const activeCategory = ref(-1);
const sidebarHidden = ref(true);
const currentViewState = ref('subtopics');
const mainMode = ref('grid');
const activeTemplateName = ref('');
const previewTitle = ref('');
const previewSubtitle = ref('');
const previewModalVisible = ref(false);
const accountMenuVisible = ref(false);
const profileModalVisible = ref(false);
const passwordModalVisible = ref(false);
const helpCenterVisible = ref(false);
const activeHelpTab = ref('guide');
const logoutConfirmVisible = ref(false);
const logoutSubmitting = ref(false);
const passwordSubmitting = ref(false);
const templateDetailLoading = ref(false);
const startEditingLoading = ref(false);
const templateDownloadVisible = ref(false);
const templateDownloadCanceling = ref(false);
const templateDownloadTitle = ref('');
const templateDownloadStatus = ref('');
const templateDownloadProgress = ref(0);
const templateDownloadCancelRequested = ref(false);
const activeDownloadId = ref('');
const activeTemplateId = ref('');
const activeTemplateExportCredit = ref(0);
const activeTemplateLocalInfo = ref(null);
const activeTemplateDemoSource = ref('');
const activeProjectDir = ref('');
const activeBackendProjectId = ref('');
const activeProjectExported = ref(false);
const editingFromDraftLibrary = ref(false);
const favoriteTemplateIds = ref(new Set());
const favoriteUpdatingIds = ref(new Set());
const timelineCollapsed = ref(true);
const showFinishedControls = ref(false);
const selectedVideoName = ref('');
const selectedVideoKey = ref('');
const selectedVideoAssetId = ref('');
const selectedStyleName = ref('');
const defaultSubtitleText = '';
const subtitleText = ref(defaultSubtitleText);
const subtitleApplying = ref(false);
const timelinePulse = ref(false);

// 工程库、收藏夹和账户表单状态。
const draftLibraryVisible = ref(false);
const finishedLibraryVisible = ref(false);
const draftFilter = ref('all');
const activeFavoriteFilter = ref('all');
const editingDraftId = ref('');
const draftEditTitle = ref('');
const draftRenamingId = ref('');
const draftBatchDeleteMode = ref(false);
const draftDeleteConfirmVisible = ref(false);
const draftDeleting = ref(false);
const draftOpeningId = ref('');
const selectedDraftProjectIds = ref(new Set());
const draftTitleInputRefs = new Map();
const passwordForm = reactive({
  oldPassword: '',
  newPassword: '',
  confirmPassword: '',
});

// 导出、素材替换确认及全局交互句柄。
const exportModalVisible = ref(false);
const exportState = ref('confirm');
const exportProgress = ref(0);
const exportStatus = ref('正在渲染视频文件...');
const exportRunning = ref(false);
const exportSelectedDir = ref('');
const exportOutputPath = ref('');
const defaultTemplateExportConfirmVisible = ref(false);
const importOverwriteConfirmVisible = ref(false);
const pendingImportSegment = ref(null);
let exportInterval = null;
let timelineMoveHandler = null;
let timelineUpHandler = null;
let timelinePlayheadMoveHandler = null;
let timelinePlayheadUpHandler = null;
let modalPreviewProgressMoveHandler = null;
let modalPreviewProgressUpHandler = null;
let playerResizeFrame = null;
let playerResizeTimer = null;
let playerResizeObserver = null;
let offsetPersistTimer = null;
let projectUpdateTimer = null;
let pendingProjectUpdate = null;
const canceledTemplateDownloadIds = new Set();
const VIDEO_FRAME_REVEAL_TIME = 0.001;

// 播放器 DOM 引用、播放状态和拖动状态。
const mainVideoRef = ref(null);
const modalVideoRef = ref(null);
const playerStageRef = ref(null);
const playerWrapperRef = ref(null);
const timelineTrackRef = ref(null);
const timelineTrackWidth = ref(0);
const timelineRulerRef = ref(null);
const timelineRulerWidth = ref(600);
const timelineDragging = ref(false);
const timelinePlayheadDragging = ref(false);
const playerPaused = ref(true);
const playerMuted = ref(false);
const playerSpeed = ref(1);
const playerProgress = ref(0);
const playerCurrentTime = ref(0);
const timelinePlayheadTime = ref(0);
const timelinePreviewSeeking = ref(false);
const playerTimeLabel = ref('00:00 / 00:00');
const modalPaused = ref(true);
const modalProgress = ref(0);
const modalPlaybackRate = ref(1);
const modalPreviewProgressDragging = ref(false);
const selectedVideoDuration = ref('00:00');
const selectedVideoSource = ref('');
const importedVideoObjectUrls = new Set();

// 时间线选区使用秒作为统一单位。
const timeline = reactive({
  totalDuration: 50,
  selectedDuration: 20,
  startTime: 0,
});

const draftProjects = ref([]);
const segmentImportState = reactive({});
const videoTimelineStateCache = reactive({});

// 页面展示数据与交互权限的派生状态。
const sidebarContextLabel = computed(() => {
  if (currentViewState.value === 'finished') return '成片素材';
  if (currentViewState.value === 'segments') return '';
  if (currentViewState.value === 'import') return '';
  return '';
});
const sidebarToggleVisible = computed(
  () => activeCategory.value >= 0 || currentViewState.value !== 'subtopics',
);
const timelineToggleVisible = computed(() => mainMode.value === 'player');
const selectedTemplateThemeName = computed(() => {
  const category = categories.value[activeCategory.value] || {};
  return category.categoryName || category.categoryId || '';
});
const hasTemplateSearchKeyword = computed(
  () => templateSearchKeyword.value.trim().length > 0,
);
const activeFavoriteKey = computed(() => String(activeTemplateId.value || ''));
const previewFavorited = computed(
  () =>
    Boolean(activeFavoriteKey.value) &&
    favoriteTemplateIds.value.has(activeFavoriteKey.value),
);
const userInfoRevision = ref(0);
const storedUserProfile = computed(() => {
  userInfoRevision.value;
  return getStoredUserProfile();
});
const accountDisplayName = computed(() => {
  const profile = storedUserProfile.value;
  return profile.phone || '--';
});
const accountVersionName = computed(() => {
  const profile = storedUserProfile.value;
  return profile.nickName || '--';
});
const accountTenantName = computed(() => {
  const profile = storedUserProfile.value;
  return profile.renterName || '--';
});
const accountBalance = computed(() => {
  const profile = storedUserProfile.value;
  return formatAccountBalance(profile.creditBalance);
});
const canExport = computed(
  () =>
    currentViewState.value === 'import' &&
    mainMode.value === 'player' &&
    Boolean(activeProjectDir.value) &&
    Boolean(activeTemplateLocalInfo.value?.templateFilePath),
);
const sidebarTitle = computed(() => {
  if (currentViewState.value === 'finished') return '已导入视频';
  if (currentViewState.value === 'segments') return activeTemplateName.value;
  if (currentViewState.value === 'import') return '素材导入';
  return '工作台';
});
const visibleSegments = computed(() => activeTemplateSegments.value);
const importSegments = computed(() =>
  visibleSegments.value.map((segment, index) => {
    const id = `${activeTemplateName.value || 'template'}-${index}-${segment.name}`;
    const state = segmentImportState[id] || {};
    const count = Number(segment.count) || 0;
    const videos = state.videos || [];
    const remainingCount = Math.max(0, count - videos.length);

    return {
      ...segment,
      id,
      count,
      imported: Boolean(state.imported),
      videos,
      remainingCount,
    };
  }),
);
const visibleDraftProjects = computed(() =>
  draftFilter.value === 'all'
    ? draftProjects.value
    : draftProjects.value.filter(
        (project) => project.status === draftFilter.value,
      ),
);
const favoriteLibraryItems = computed(() => {
  if (activeFavoriteFilter.value === 'all') {
    return favoriteTemplates.value;
  }

  return favoriteTemplates.value.filter(
    (template) =>
      getTemplateCategoryKey(template) === activeFavoriteFilter.value,
  );
});
const favoriteCategoryFilters = computed(() => {
  const map = new Map();
  const favoritedTemplates = favoriteTemplates.value;

  for (const template of favoritedTemplates) {
    const key = getTemplateCategoryKey(template);
    if (!key) continue;

    const current = map.get(key) || {
      key,
      label: getTemplateCategoryLabel(template, key),
      count: 0,
    };
    current.count += 1;
    map.set(key, current);
  }

  return [
    {
      key: 'all',
      label: '全部',
      count: favoritedTemplates.length,
    },
    ...Array.from(map.values()),
  ];
});
const timelineSelectionStyle = computed(() => {
  const totalDuration = Math.max(1, Number(timeline.totalDuration) || 1);
  const selectedDuration = Math.min(
    totalDuration,
    Math.max(0, Math.floor(Number(timeline.selectedDuration) || 0)),
  );
  const maxStart = Math.max(0, totalDuration - selectedDuration);
  const startTime = Math.min(
    maxStart,
    Math.max(0, Number(timeline.startTime) || 0),
  );
  const trackWidth = Math.max(0, timelineTrackWidth.value);
  if (!trackWidth) {
    return {
      left: '0px',
      width: `${(selectedDuration / totalDuration) * 100}%`,
    };
  }

  const width = Math.min(
    trackWidth,
    trackWidth * (selectedDuration / totalDuration),
  );
  const maxLeft = Math.max(0, trackWidth - width);
  const left = maxStart > 0 ? maxLeft * (startTime / maxStart) : 0;

  return {
    left: `${left}px`,
    width: `${width}px`,
    transform: 'translateY(-50%)',
  };
});
const timelinePlayheadPercent = computed(() => {
  const totalDuration = Math.max(1, Number(timeline.totalDuration) || 1);
  const currentTime = Math.min(
    totalDuration,
    Math.max(0, Number(timelinePlayheadTime.value) || 0),
  );

  return `${(currentTime / totalDuration) * 100}%`;
});
const timelineRangeLabel = computed(() => {
  const endTime = timeline.startTime + timeline.selectedDuration;
  return `${formatTimelineTick(timeline.startTime)} - ${formatTimelineTick(endTime)}`;
});
const timelineSelectedDurationLabel = computed(() =>
  formatPlayerTime(timeline.selectedDuration),
);
function getNiceTickStep(start, stop, count) {
  const rawStep = Math.abs(stop - start) / Math.max(1, count);
  if (!Number.isFinite(rawStep) || rawStep <= 0) return 1;

  const power = Math.floor(Math.log10(rawStep));
  const magnitude = 10 ** power;
  const error = rawStep / magnitude;
  const factor =
    error >= Math.sqrt(50)
      ? 10
      : error >= Math.sqrt(10)
        ? 5
        : error >= Math.sqrt(2)
          ? 2
          : 1;

  return factor * magnitude;
}

function buildTimelineTicks(totalDuration, step, majorStep, includeMajor) {
  const epsilon = step / 1000;
  const ticks = [];
  const count = Math.floor((totalDuration + epsilon) / step);

  for (let index = 0; index <= count; index += 1) {
    const value = index * step;
    const majorRatio = value / majorStep;
    const isMajor = Math.abs(majorRatio - Math.round(majorRatio)) < 0.0001;
    if (includeMajor === isMajor) {
      ticks.push({
        value,
        left: `${(value / totalDuration) * 100}%`,
      });
    }
  }

  return ticks;
}

const timelineRulerScale = computed(() => {
  const totalDuration = Math.max(1, Number(timeline.totalDuration) || 1);
  const targetMajorCount = Math.max(
    2,
    Math.min(12, Math.floor(timelineRulerWidth.value / 84)),
  );
  const majorStep = getNiceTickStep(0, totalDuration, targetMajorCount);

  return {
    totalDuration,
    majorStep,
    minorStep: majorStep / 5,
  };
});
const timelineRulerMajorTicks = computed(() => {
  const { totalDuration, majorStep } = timelineRulerScale.value;
  return buildTimelineTicks(totalDuration, majorStep, majorStep, true);
});
const timelineRulerMinorTicks = computed(() => {
  const { totalDuration, majorStep, minorStep } = timelineRulerScale.value;
  return buildTimelineTicks(totalDuration, minorStep, majorStep, false);
});
const selectedClipTitle = computed(() => `${selectedVideoName.value}`);

// 页面级导航和主视图切换。
function statusMeta(status) {
  return status === 'exported'
    ? {
        label: '已导出',
        className: 'bg-surface-container-high text-on-surface',
        detail: '已完成导出',
      }
    : {
        label: '编辑中',
        className: 'bg-electric-blue text-white',
        detail: '工程继续编辑',
      };
}

function goHome() {
  resetDraftBatchDelete();
  activeCategory.value = -1;
  templateSearchKeyword.value = '';
  currentViewState.value = 'subtopics';
  mainMode.value = 'grid';
  sidebarHidden.value = true;
  timelineCollapsed.value = true;
  showFinishedControls.value = false;

  previewModalVisible.value = false;
  draftLibraryVisible.value = false;
  finishedLibraryVisible.value = false;
  profileModalVisible.value = false;
  passwordModalVisible.value = false;
  helpCenterVisible.value = false;
  accountMenuVisible.value = false;
  exportModalVisible.value = false;
  defaultTemplateExportConfirmVisible.value = false;
  importOverwriteConfirmVisible.value = false;

  activeTemplateName.value = '';
  activeTemplateId.value = '';
  activeTemplateExportCredit.value = 0;
  activeTemplateLocalInfo.value = null;
  activeTemplateSegments.value = [];
  activeTemplateDemoSource.value = '';
  activeProjectDir.value = '';
  activeBackendProjectId.value = '';
  activeProjectExported.value = false;
  editingFromDraftLibrary.value = false;
  selectedVideoSource.value = '';

  resetModalPreviewVideo();
  resetMainPlayer();
  loadRecommendedTemplates();
  nextTick(schedulePlayerResize);
}

async function selectCategory(index) {
  draftLibraryVisible.value = false;
  finishedLibraryVisible.value = false;
  resetDraftBatchDelete();
  activeCategory.value = index;
  showFinishedControls.value = false;
  currentViewState.value = 'subtopics';
  mainMode.value = 'grid';
  timelineCollapsed.value = true;
  previewModalVisible.value = false;
  sidebarHidden.value = false;
  resetModalPreviewVideo();
  resetMainPlayer();
  await loadTemplatesByCategory(categories.value[index]);
}

function toggleSidebar(show) {
  sidebarHidden.value =
    typeof show === 'boolean' ? !show : !sidebarHidden.value;
  nextTick(schedulePlayerResize);
}

function openPreview(title, subtitle) {
  showFinishedControls.value = false;
  activeTemplateId.value = '';
  activeTemplateExportCredit.value = 0;
  activeTemplateLocalInfo.value = null;
  activeTemplateDemoSource.value = '';
  activeProjectDir.value = '';
  activeBackendProjectId.value = '';
  activeProjectExported.value = false;
  activeTemplateName.value = title;
  previewTitle.value = title;
  previewSubtitle.value = subtitle;
  currentViewState.value = 'segments';
  previewModalVisible.value = true;
  sidebarHidden.value = false;
  resetModalPreviewVideo();
}

// 将模板素材统计映射为预览页面可用结构。
function getTemplateMaterialSummary(segments) {
  const styleCount = segments.length;
  const videoCount = segments.reduce(
    (total, segment) => total + (Number(segment.count) || 0),
    0,
  );

  return `${videoCount}个视频素材 · ${styleCount}类素材风格`;
}

function enterTemplatePreview(topic, templateId, localInfo) {
  const title = topic.title || templateId;
  const segments = parseTemplateSegments(localInfo.xmlContent);
  const demoPath = parseTemplateDemoPath(localInfo.xmlContent);

  activeTemplateSegments.value = segments;
  openPreview(title, getTemplateMaterialSummary(segments));
  activeTemplateId.value = templateId;
  const exportCredit = Number(topic.exportCredit);
  activeTemplateExportCredit.value = Number.isFinite(exportCredit)
    ? exportCredit
    : 0;
  activeTemplateLocalInfo.value = localInfo;
  activeTemplateDemoSource.value = resolveTemplateVideoSource(demoPath);
  activeProjectDir.value = '';
  activeBackendProjectId.value = '';
  activeProjectExported.value = false;
  editingFromDraftLibrary.value = false;
  nextTick(resetModalPreviewVideo);
}

async function openTemplatePreview(topic) {
  if (templateDetailLoading.value) {
    return;
  }

  const templateId = topic.templateId || topic.id;
  const localTemplateId = String(templateId || '');

  if (!templateId) {
    systemMessage.error('模板 ID 不存在');
    return;
  }

  templateDetailLoading.value = true;
  templateDownloadCanceling.value = false;
  templateDownloadCancelRequested.value = false;
  templateDownloadTitle.value = topic.title || templateId;
  templateDownloadStatus.value = '';
  templateDownloadProgress.value = 0;
  activeDownloadId.value = '';
  let unlistenProgress = null;
  let downloadId = '';

  try {
    const detailResponse = await getTemplateDetail({ templateId });
    const detail = getResponsePayload(detailResponse) || {};
    const templateFileUrl = detail.xmlPath || detail.templateFileUrl;
    const materialPackageUrl = detail.assetsPath || detail.materialPackageUrl;
    const templateVersion = String(detail.version ?? '');

    if (!templateFileUrl || !materialPackageUrl) {
      throw new Error('模板详情缺少下载地址');
    }

    const cachedInfo = await invoke('get_cached_template_assets', {
      templateId: localTemplateId,
      templateVersion,
    });
    if (cachedInfo) {
      enterTemplatePreview(topic, localTemplateId, cachedInfo);
      return;
    }

    downloadId = `${localTemplateId}-${Date.now()}`;
    activeDownloadId.value = downloadId;
    templateDownloadVisible.value = true;
    templateDownloadStatus.value = '正在获取模板详情...';

    unlistenProgress = await listen('template-download-progress', (event) => {
      const payload = event.payload || {};
      if (payload.downloadId !== downloadId) return;
      if (canceledTemplateDownloadIds.has(downloadId)) return;

      templateDownloadProgress.value = Number(payload.progress) || 0;
      templateDownloadStatus.value =
        payload.status || templateDownloadStatus.value;
    });

    if (templateDownloadCancelRequested.value) {
      throw new Error('Download canceled');
    }

    templateDownloadStatus.value = '正在下载模板 XML 和素材包...';
    templateDownloadProgress.value = Math.max(
      templateDownloadProgress.value,
      3,
    );
    const localInfo = await invoke('prepare_template_assets', {
      templateId: localTemplateId,
      templateVersion,
      templateFileUrl,
      materialPackageUrl,
      apiBaseUrl: TEMPLATE_DOWNLOAD_BASE_URL,
      downloadId,
    });
    if (canceledTemplateDownloadIds.has(downloadId)) {
      throw new Error('Download canceled');
    }

    templateDownloadStatus.value = '正在解析模板片段...';
    templateDownloadProgress.value = Math.max(
      templateDownloadProgress.value,
      98,
    );
    enterTemplatePreview(topic, localTemplateId, localInfo);
  } catch (error) {
    const message = error?.message || String(error || '');
    const isCanceled =
      message.includes('取消') || message.toLowerCase().includes('canceled');
    if (!isCanceled) {
      systemMessage.error(message || '模板详情加载失败');
    }
  } finally {
    const isCurrentDownload =
      !downloadId || activeDownloadId.value === downloadId;
    if (isCurrentDownload) {
      templateDetailLoading.value = false;
      templateDownloadVisible.value = false;
      templateDownloadCanceling.value = false;
      templateDownloadCancelRequested.value = false;
      templateDownloadStatus.value = '';
      templateDownloadProgress.value = 0;
      activeDownloadId.value = '';
    }
    if (downloadId) {
      canceledTemplateDownloadIds.delete(downloadId);
    }
    if (unlistenProgress) {
      unlistenProgress();
    }
  }
}

// 取消模板下载并同步清理前端下载状态。
function cancelTemplateDownload() {
  if (!activeDownloadId.value || templateDownloadCanceling.value) return;

  const downloadId = activeDownloadId.value;
  templateDownloadCancelRequested.value = true;
  canceledTemplateDownloadIds.add(downloadId);
  templateDownloadVisible.value = false;
  templateDownloadCanceling.value = false;
  templateDetailLoading.value = false;
  templateDownloadStatus.value = '';
  templateDownloadProgress.value = 0;
  activeDownloadId.value = '';

  invoke('cancel_template_download', {
    downloadId,
  }).catch((error) => {
    console.error('[template-download] cancel failed', error);
  });
}

function hidePreviewModal() {
  if (startEditingLoading.value) {
    return;
  }

  previewModalVisible.value = false;
  resetModalPreviewVideo();
  if (currentViewState.value === 'segments') {
    currentViewState.value = 'subtopics';
  }
}

// 模板收藏状态在侧边列表、推荐列表和收藏夹之间保持同步。
function getTemplateFavoriteKey(template) {
  const id = template?.templateId || template?.id || '';
  return id ? String(id) : '';
}

function getTemplateCategoryKey(template) {
  const key =
    template?.templateCategoryId ||
    template?.categoryId ||
    template?.category_id ||
    template?.type ||
    '';

  return key ? String(key) : '';
}

function getTemplateCategoryLabel(template, key) {
  const category = categories.value.find(
    (item) =>
      String(item.categoryId || item.id || '') === key ||
      item.categoryName === template?.categoryName,
  );

  return (
    template?.categoryName ||
    template?.templateCategoryName ||
    category?.categoryName ||
    key ||
    '未分类'
  );
}

function isTemplateFavorited(template) {
  const key = getTemplateFavoriteKey(template);
  return Boolean(key) && favoriteTemplateIds.value.has(key);
}

function isTemplateFavoriteUpdating(template) {
  const key = getTemplateFavoriteKey(template);
  return Boolean(key) && favoriteUpdatingIds.value.has(key);
}

function setTemplateFavoriteState(templateId, favorited) {
  const key = String(templateId || '');
  if (!key) return;

  const nextFavorites = new Set(favoriteTemplateIds.value);
  if (favorited) {
    nextFavorites.add(key);
  } else {
    nextFavorites.delete(key);
  }
  favoriteTemplateIds.value = nextFavorites;

  const patchTemplate = (template) =>
    getTemplateFavoriteKey(template) === key
      ? { ...template, favorite: favorited ? 1 : 0, favorited }
      : template;

  subtopics.value = subtopics.value.map(patchTemplate);
  recommendationCards.value = recommendationCards.value.map(patchTemplate);
  favoriteTemplates.value = favorited
    ? favoriteTemplates.value.map(patchTemplate)
    : favoriteTemplates.value.filter(
        (template) => getTemplateFavoriteKey(template) !== key,
      );
}

function syncTemplateFavoriteStates(templates) {
  const nextFavorites = new Set(favoriteTemplateIds.value);

  for (const template of templates) {
    const key = getTemplateFavoriteKey(template);
    if (!key) continue;

    if (Number(template.favorite) === 1 || template.favorited === true) {
      nextFavorites.add(key);
    } else {
      nextFavorites.delete(key);
    }
  }

  favoriteTemplateIds.value = nextFavorites;
}

function addFavoriteTemplateToLibrary(template) {
  const key = getTemplateFavoriteKey(template);
  if (
    !key ||
    !(template?.title || template?.templateName) ||
    favoriteTemplates.value.some((item) => getTemplateFavoriteKey(item) === key)
  ) {
    return;
  }

  favoriteTemplates.value = [
    {
      ...template,
      favorite: 1,
      favorited: true,
    },
    ...favoriteTemplates.value,
  ];
}

async function loadFavoriteTemplates() {
  favoritesLoading.value = true;

  try {
    const response = await getFavoriteTemplates({
      renter_id: getStoredTenantId(),
      userId: getStoredUserId(),
    });
    const templates = getResponseList(response).map((template, index) => ({
      ...mapTemplateTopic(template, index),
      favorite: 1,
      favorited: true,
    }));

    favoriteTemplates.value = templates;
    syncTemplateFavoriteStates(templates);
  } catch (error) {
    favoriteTemplates.value = [];
    systemMessage.error(error?.message || '收藏模板加载失败');
  } finally {
    favoritesLoading.value = false;
  }
}

async function toggleTemplateFavorite(template) {
  const key = getTemplateFavoriteKey(template);
  if (!key || isTemplateFavoriteUpdating(template)) return;

  const currentlyFavorited = isTemplateFavorited(template);
  const numericTemplateId = Number(template.templateId || template.id);
  const requestTemplateId = Number.isFinite(numericTemplateId)
    ? numericTemplateId
    : template.templateId || template.id;
  const nextUpdating = new Set(favoriteUpdatingIds.value);
  nextUpdating.add(key);
  favoriteUpdatingIds.value = nextUpdating;

  try {
    const response = await favoriteTemplate({
      action: currentlyFavorited ? 1 : 0,
      renter_id: getStoredTenantId(),
      templateId: requestTemplateId,
      userId: getStoredUserId(),
    });

    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '收藏操作失败');
    }

    setTemplateFavoriteState(key, !currentlyFavorited);
    if (!currentlyFavorited) {
      addFavoriteTemplateToLibrary(template);
    }
    systemMessage.success(
      response?.msg || (currentlyFavorited ? '已取消收藏' : '收藏成功'),
    );
  } catch (error) {
    systemMessage.error(error?.message || '收藏操作失败');
  } finally {
    const latestUpdating = new Set(favoriteUpdatingIds.value);
    latestUpdating.delete(key);
    favoriteUpdatingIds.value = latestUpdating;
  }
}

function togglePreviewFavorite() {
  if (!activeTemplateId.value) return;

  toggleTemplateFavorite({
    id: activeTemplateId.value,
    templateId: activeTemplateId.value,
  });
}

// 确认模板后创建后端工程，并切换到素材导入工作流。
async function confirmSelection() {
  if (startEditingLoading.value) {
    return;
  }

  startEditingLoading.value = true;

  try {
    const started = await startEditing();
    if (started) {
      previewModalVisible.value = false;
      resetModalPreviewVideo();
    }
  } finally {
    startEditingLoading.value = false;
  }
}

function normalizeBackendId(value) {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue : value;
}

async function syncProjectTemplateUpdate(payload) {
  try {
    const response = await updateProject(payload);
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '工程信息同步失败');
    }
  } catch (error) {
    console.error('[project] template update sync failed:', error);
    systemMessage.error(error?.message || '工程信息同步失败');
  }
}

function scheduleProjectTemplateUpdate(projectXml) {
  if (!projectXml || !activeBackendProjectId.value || !activeTemplateId.value) {
    return;
  }

  pendingProjectUpdate = {
    projectId: normalizeBackendId(activeBackendProjectId.value),
    projectName: activeTemplateName.value || previewTitle.value || '未命名工程',
    projectXml,
    templateId: normalizeBackendId(activeTemplateId.value),
  };

  if (projectUpdateTimer) {
    window.clearTimeout(projectUpdateTimer);
  }

  projectUpdateTimer = window.setTimeout(() => {
    const payload = pendingProjectUpdate;
    projectUpdateTimer = null;
    pendingProjectUpdate = null;
    if (payload) {
      syncProjectTemplateUpdate(payload);
    }
  }, 5000);
}

function flushPendingProjectTemplateUpdate() {
  if (projectUpdateTimer) {
    window.clearTimeout(projectUpdateTimer);
    projectUpdateTimer = null;
  }

  const payload = pendingProjectUpdate;
  pendingProjectUpdate = null;
  if (payload) {
    syncProjectTemplateUpdate(payload);
  }
}

async function createBackendProjectIfNeeded() {
  if (activeBackendProjectId.value) {
    return true;
  }

  const renterId = getStoredTenantId();
  const userId = getStoredUserId();
  const templateId = activeTemplateId.value;
  const projectName =
    activeTemplateName.value || previewTitle.value || `工程 ${Date.now()}`;

  if (!renterId || !userId || !templateId) {
    systemMessage.error('用户或模板信息不完整，无法创建工程');
    return false;
  }

  try {
    console.log('[project] creating backend project', {
      projectName,
      renter_id: renterId,
      templateId,
      userId,
    });
    const response = await createProject({
      projectName,
      renter_id: renterId,
      templateId: normalizeBackendId(templateId),
      userId: normalizeBackendId(userId),
    });

    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '工程创建失败');
    }

    const payload = getResponsePayload(response) || {};
    activeBackendProjectId.value = payload.projectId || payload.id || '';
    if (!activeBackendProjectId.value) {
      throw new Error('创建工程成功，但未返回工程 ID');
    }
    activeProjectExported.value = false;
    console.log('[project] backend project created', {
      projectId: activeBackendProjectId.value,
    });
    return true;
  } catch (error) {
    console.error('[project] backend project create failed:', error);
    systemMessage.error(error?.message || '工程创建失败');
    return false;
  }
}

async function startEditing() {
  editingFromDraftLibrary.value = false;
  const backendProjectCreated = await createBackendProjectIfNeeded();
  if (!backendProjectCreated) {
    return false;
  }

  if (activeTemplateId.value && !activeProjectDir.value) {
    try {
      const workspace = await invoke('create_project_workspace', {
        projectId: String(activeBackendProjectId.value),
        templateId: activeTemplateId.value,
      });
      activeProjectDir.value = workspace.projectDir;
      scheduleProjectTemplateUpdate(workspace.projectXml);
    } catch (error) {
      systemMessage.error(error?.message || '项目目录创建失败');
      return false;
    }
  }

  loadMyProjects();

  clearProjectEditingState();
  subtitleText.value = parseTemplateSubtitleText(getActiveTemplateXmlContent());
  await initializeDefaultTemplateAssets();
  try {
    await initializeProjectAssetOffsets();
  } catch (error) {
    systemMessage.error(error?.message || '工程素材位置初始化失败');
    return false;
  }
  currentViewState.value = 'import';
  mainMode.value = 'player';
  timelineCollapsed.value = false;
  showFinishedControls.value = false;
  await nextTick();

  const firstSegmentWithVideo = importSegments.value.find(
    (segment) => segment.videos.length > 0,
  );
  if (firstSegmentWithVideo) {
    selectVideoForTimeline(
      firstSegmentWithVideo.videos[0],
      firstSegmentWithVideo.name,
    );
  } else {
    selectVideoForTimeline('视频 1', '视频轨道 V1');
  }
  schedulePlayerResize();

  return true;
}

function handleSidebarBack() {
  if (currentViewState.value === 'import') {
    flushPendingProjectTemplateUpdate();
    activeProjectDir.value = '';
    activeBackendProjectId.value = '';
    activeProjectExported.value = false;
    editingFromDraftLibrary.value = false;
    clearProjectEditingState();
    showFinishedControls.value = false;
    currentViewState.value = 'segments';
    mainMode.value = 'grid';
    timelineCollapsed.value = true;
    previewModalVisible.value = true;
  } else if (currentViewState.value === 'segments') {
    currentViewState.value = 'subtopics';
    previewModalVisible.value = false;
    resetModalPreviewVideo();
  }
}

// 素材导入状态与本地路径处理。
function getSegmentImportState(segmentId) {
  return segmentImportState[segmentId] || { imported: false, videos: [] };
}

function normalizeComparablePath(filePath) {
  return String(filePath || '')
    .replaceAll('\\', '/')
    .replace(/\/+/g, '/')
    .replace(/\/$/g, '')
    .toLowerCase();
}

function isProjectImportedVideo(video) {
  if (!video) return false;
  if (video.projectFilepath) return true;

  const projectDir = normalizeComparablePath(activeProjectDir.value);
  const localPath = normalizeComparablePath(video.localPath);

  return Boolean(
    projectDir && localPath && localPath.startsWith(`${projectDir}/`),
  );
}

function isSegmentFullyImported(segment) {
  const videos = segment?.videos || [];
  const targetCount = Number(segment?.count) || 0;

  return (
    targetCount > 0 &&
    videos.length >= targetCount &&
    videos.slice(0, targetCount).every(isProjectImportedVideo)
  );
}

function hasImportedVideoInSegment(segment) {
  return (segment?.videos || []).some(isProjectImportedVideo);
}

function hasDefaultTemplateVideos() {
  return importSegments.value.some((segment) =>
    segment.videos.some(
      (video) => Boolean(video?.localPath) && !isProjectImportedVideo(video),
    ),
  );
}

function getFileNameFromPath(filePath) {
  return (
    String(filePath || '')
      .replaceAll('\\', '/')
      .split('/')
      .filter(Boolean)
      .pop() || '视频素材'
  );
}

function getDisplayVideoName(filePath) {
  return getFileNameFromPath(filePath).replace(/^[0-9a-f]{16}_/i, '');
}

function joinLocalPath(basePath, relativePath) {
  const separator = String(basePath || '').includes('\\') ? '\\' : '/';
  const base = String(basePath || '').replace(/[\\/]+$/g, '');
  let relative = String(relativePath || '')
    .replaceAll('\\', '/')
    .replace(/^[\\/]+|[\\/]+$/g, '');

  if (separator === '\\') {
    relative = relative.replaceAll('/', '\\');
  }

  return [base, relative].filter(Boolean).join(separator);
}

function resolveTemplateAssetPath(filePath) {
  const localInfo = activeTemplateLocalInfo.value || {};
  const normalizedPath = String(filePath || '')
    .replaceAll('\\', '/')
    .replace(/^\.\//, '');

  if (!normalizedPath) return '';
  if (/^[a-zA-Z]:\//.test(normalizedPath) || normalizedPath.startsWith('/')) {
    return normalizedPath;
  }
  if (normalizedPath.startsWith('template/assets/')) {
    return joinLocalPath(
      localInfo.assetsDir,
      normalizedPath.slice('template/assets/'.length),
    );
  }
  if (normalizedPath.startsWith('assets/')) {
    return joinLocalPath(
      localInfo.assetsDir,
      normalizedPath.slice('assets/'.length),
    );
  }
  if (normalizedPath.startsWith('template/')) {
    return joinLocalPath(
      localInfo.templateDir,
      normalizedPath.slice('template/'.length),
    );
  }

  return joinLocalPath(localInfo.templateDir, normalizedPath);
}

function resolveTemplateVideoSource(filePath) {
  const localPath = resolveTemplateAssetPath(filePath);
  return localPath ? convertFileSrc(localPath) : '';
}

// 从模板 XML 中读取素材区域，后续解析同时提供 DOM 和文本兜底方案。
function getActiveTemplateXmlContent() {
  return activeTemplateLocalInfo.value?.xmlContent || '';
}

function getDirectChildElements(parent, tagName) {
  const lowerTagName = tagName.toLowerCase();

  return Array.from(parent.children).filter(
    (child) => child.tagName.toLowerCase() === lowerTagName,
  );
}

function normalizeTemplateDurationSeconds(durationValue) {
  const duration = normalizeTemplateDurationMs(durationValue);
  if (!duration) return 0;

  return duration / 1000;
}

function normalizeTemplateDurationMs(durationValue) {
  const duration = Number(String(durationValue || '').trim());
  return Number.isFinite(duration) && duration > 0 ? duration : 0;
}

function getAreaSourceDurationInfoFromElement(area) {
  const source = getDirectChildElements(area, 'source')[0];
  const durationRaw = source
    ? getDirectChildElements(source, 'duration')[0]?.textContent?.trim() || ''
    : '';

  return {
    durationRaw,
    durationMs: normalizeTemplateDurationMs(durationRaw),
    durationSeconds: normalizeTemplateDurationSeconds(durationRaw),
  };
}

function findTemplateAreaMatchesFromDom(xmlContent, assetId) {
  const xml = new DOMParser().parseFromString(xmlContent, 'text/xml');
  if (xml.querySelector('parsererror')) return null;

  const matches = [];
  const clips = Array.from(xml.querySelectorAll('clips'));
  for (const clipsElement of clips) {
    const clipElements = getDirectChildElements(clipsElement, 'clip');
    for (const clip of clipElements) {
      const areas = getDirectChildElements(clip, 'area');
      for (const area of areas) {
        if (area.getAttribute('asset-id') === assetId) {
          matches.push({
            clipsId: clipsElement.getAttribute('id') || '',
            clipId: clip.getAttribute('id') || '',
            areaId: area.getAttribute('id') || '',
            clipsAssetId: area.getAttribute('asset-id') || '',
            ...getAreaSourceDurationInfoFromElement(area),
          });
        }
      }
    }
  }

  return matches;
}

function findTemplateAreaMatchesFromText(xmlContent, assetId) {
  const clipsMatches = Array.from(
    xmlContent.matchAll(/<clips\b([^>]*)>([\s\S]*?)<\/clips>/gi),
  );
  const matches = [];

  for (const clipsMatch of clipsMatches) {
    const clipsAttributes = clipsMatch[1] || '';
    const clipsBody = clipsMatch[2] || '';
    const clipMatches = Array.from(
      clipsBody.matchAll(/<clip\b([^>]*)>([\s\S]*?)<\/clip>/gi),
    );

    for (const clipMatch of clipMatches) {
      const clipAttributes = clipMatch[1] || '';
      const clipBody = clipMatch[2] || '';
      const areaMatches = Array.from(
        clipBody.matchAll(/<area\b([^>]*)>([\s\S]*?)<\/area>/gi),
      );

      for (const areaMatch of areaMatches) {
        const areaAttributes = areaMatch[1] || '';
        const areaBody = areaMatch[2] || '';
        const clipsAssetId = getAttributeValue(areaAttributes, 'asset-id');
        if (clipsAssetId !== assetId) {
          continue;
        }

        const durationRaw = getElementText(
          getElementText(areaBody, 'source'),
          'duration',
        );

        matches.push({
          clipsId: getAttributeValue(clipsAttributes, 'id'),
          clipId: getAttributeValue(clipAttributes, 'id'),
          areaId: getAttributeValue(areaAttributes, 'id'),
          clipsAssetId,
          durationRaw,
          durationMs: normalizeTemplateDurationMs(durationRaw),
          durationSeconds: normalizeTemplateDurationSeconds(durationRaw),
        });
      }
    }
  }

  return matches;
}

function findTemplateAreaMatches(assetId) {
  const xmlContent = getActiveTemplateXmlContent();
  const normalizedAssetId = String(assetId || '').trim();
  if (!xmlContent || !normalizedAssetId) return [];

  return (
    findTemplateAreaMatchesFromDom(xmlContent, normalizedAssetId) ??
    findTemplateAreaMatchesFromText(xmlContent, normalizedAssetId)
  );
}

// 为时间线选区查找当前素材对应的模板时长约束。
function findTemplateAreaMatch(assetId) {
  return findTemplateAreaMatches(assetId)[0] || null;
}

function findLongestTemplateAreaMatch(assetId) {
  return findTemplateAreaMatches(assetId).reduce((longest, match) => {
    const duration = Number(match.durationMs) || 0;
    const longestDuration = Number(longest?.durationMs) || 0;
    return duration > longestDuration ? match : longest;
  }, null);
}

function findTemplateAreaDurationSeconds(assetId) {
  return findLongestTemplateAreaMatch(assetId)?.durationSeconds || null;
}

function getTimelineSelectionDuration(videoInfo, videoDuration) {
  const templateDuration = findTemplateAreaDurationSeconds(videoInfo.assetId);

  return Number.isFinite(templateDuration) && templateDuration > 0
    ? Math.max(1, Math.floor(templateDuration))
    : Math.max(1, Math.floor(videoDuration));
}

function logTemplateAssetDurationMatch(videoInfo) {
  const materialAssetId = videoInfo.assetId || '';
  const areaMatches = findTemplateAreaMatches(materialAssetId);
  const areaMatch = findLongestTemplateAreaMatch(materialAssetId);

  console.log('[duration-track]', {
    materialAssetId,
    clipsAssetId: areaMatch?.clipsAssetId || '',
    duration: areaMatch?.durationRaw || '',
    durationSeconds: areaMatch?.durationSeconds || null,
    matched: Boolean(areaMatch),
    fallbackToVideoDuration: !areaMatch,
    videoDurationSeconds: videoInfo.durationSeconds || null,
    timelineTrackDuration: timeline.totalDuration,
    timelineSelectionDuration: timeline.selectedDuration,
    matchedAreas: areaMatches.map((match) => ({
      areaId: match.areaId,
      clipsAssetId: match.clipsAssetId,
      duration: match.durationRaw,
      durationSeconds: match.durationSeconds,
    })),
  });
}

// 切换视频前缓存时间线状态，避免不同素材之间互相覆盖选区。
function cacheCurrentVideoTimelineState() {
  const key = selectedVideoKey.value;
  if (!key) return;

  videoTimelineStateCache[key] = {
    startTime: timeline.startTime,
  };
}

function getVideoTimelineState(key) {
  return videoTimelineStateCache[key] || null;
}

async function createTemplateAssetVideo(asset, index) {
  const existingAssetIds = activeTemplateLocalInfo.value?.existingAssetIds;
  const hasExistenceSnapshot = Array.isArray(existingAssetIds);
  const exists =
    !hasExistenceSnapshot || existingAssetIds.includes(String(asset.id || ''));
  const localPath = exists ? resolveTemplateAssetPath(asset.filepath) : '';
  const source = exists ? resolveTemplateVideoSource(asset.filepath) : '';
  const metadata = source
    ? await getVideoMetadata(source)
    : { durationSeconds: 0, duration: '--' };

  return {
    id: `template-${asset.id || index}-${asset.filepath || index}`,
    name: exists
      ? getDisplayVideoName(asset.filepath) || `素材 ${index + 1}`
      : `待导入视频 ${index + 1}`,
    duration: metadata.duration,
    durationSeconds: metadata.durationSeconds,
    source,
    localPath,
    assetId: asset.id || '',
    missing: !exists,
  };
}

async function initializeDefaultTemplateAssets() {
  const segments = importSegments.value;

  for (const segment of segments) {
    const state = getSegmentImportState(segment.id);
    if (state.videos.length > 0 || !segment.defaultAssets?.length) {
      continue;
    }

    const videos = [];
    for (const [index, asset] of segment.defaultAssets.entries()) {
      videos.push(await createTemplateAssetVideo(asset, index));
    }

    segmentImportState[segment.id] = {
      imported: videos.length >= (Number(segment.count) || 0),
      videos,
    };
  }
}

function clearProjectEditingState() {
  if (offsetPersistTimer) {
    window.clearTimeout(offsetPersistTimer);
    offsetPersistTimer = null;
  }
  for (const key of Object.keys(segmentImportState)) {
    delete segmentImportState[key];
  }
  for (const key of Object.keys(videoTimelineStateCache)) {
    delete videoTimelineStateCache[key];
  }
  selectedVideoName.value = '';
  selectedVideoKey.value = '';
  selectedVideoAssetId.value = '';
  selectedVideoSource.value = '';
  selectedVideoDuration.value = '00:00';
  selectedStyleName.value = '';
  subtitleText.value = defaultSubtitleText;
  timeline.totalDuration = 50;
  timeline.selectedDuration = 20;
  timeline.startTime = 0;
  pendingImportSegment.value = null;
  importOverwriteConfirmVisible.value = false;
  resetMainPlayer();
}

function restoreProjectVideoOffsets(projectFileXml) {
  const maxOffsets = parseProjectMaxOffsets(projectFileXml);

  for (const segment of importSegments.value) {
    for (const video of segment.videos) {
      const offsetMs = maxOffsets.get(String(video.assetId || ''));
      if (Number.isFinite(offsetMs)) {
        videoTimelineStateCache[video.id] = {
          startTime: Math.max(0, offsetMs / 1000),
        };
      }
    }
  }
}

// 读取视频元数据并将用户选择的本地文件转换为可播放素材。
function getVideoMetadata(source) {
  return new Promise((resolve) => {
    const video = document.createElement('video');
    video.preload = 'metadata';
    video.onloadedmetadata = () => {
      resolve({
        durationSeconds: Number.isFinite(video.duration) ? video.duration : 0,
        duration: formatPlayerTime(video.duration || 0),
      });
    };
    video.onerror = () => {
      resolve({
        durationSeconds: 0,
        duration: '--',
      });
    };
    video.src = source;
    video.load();
  });
}

async function pickVideoPaths(multiple) {
  const selected = await openDialog({
    multiple,
    filters: [
      {
        name: '视频文件',
        extensions: ['mp4', 'mov', 'm4v', 'avi', 'mkv', 'webm'],
      },
    ],
  });

  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

async function createImportedVideoFromPath(
  filePath,
  keySuffix = '',
  assetId = '',
) {
  let localPath = filePath;
  let projectFilepath = '';

  if (activeProjectDir.value && assetId) {
    const savedAsset = await invoke('save_project_asset', {
      projectDir: activeProjectDir.value,
      assetId,
      sourcePath: filePath,
    });

    localPath = savedAsset.copiedPath || filePath;
    projectFilepath = savedAsset.projectFilepath || '';
    scheduleProjectTemplateUpdate(savedAsset.projectXml);
  }

  const source = convertFileSrc(localPath);
  const metadata = await getVideoMetadata(source);
  const suffix = keySuffix || Math.random().toString(36).slice(2);
  const importedVideo = {
    id: `imported-${Date.now()}-${suffix}`,
    name: getFileNameFromPath(filePath),
    duration: metadata.duration,
    durationSeconds: metadata.durationSeconds,
    source,
    localPath,
    assetId,
    projectFilepath,
  };

  await initializeVideoDefaultOffset(importedVideo);
  return importedVideo;
}

function revokeImportedVideo(video) {
  if (
    video?.isObjectUrl &&
    video.source &&
    importedVideoObjectUrls.has(video.source)
  ) {
    URL.revokeObjectURL(video.source);
    importedVideoObjectUrls.delete(video.source);
  }
}

async function deleteProjectAssetFiles(assetPaths) {
  if (!activeProjectDir.value || assetPaths.length === 0) return;

  await invoke('delete_project_asset_files', {
    projectDir: activeProjectDir.value,
    assetPaths,
  });
}

async function deleteReplacedProjectAssets(previousVideos, nextVideos = []) {
  const nextPaths = new Set(
    nextVideos.map((video) => video?.localPath).filter(Boolean),
  );
  const stalePaths = previousVideos
    .map((video) => video?.localPath)
    .filter((localPath) => localPath && !nextPaths.has(localPath));

  await deleteProjectAssetFiles([...new Set(stalePaths)]);
}

function buildFilledImportPaths(filePaths, count) {
  if (count <= 0 || filePaths.length === 0) return [];

  if (filePaths.length >= count) {
    return filePaths.slice(0, count);
  }

  return Array.from(
    { length: count },
    (_, index) => filePaths[index % filePaths.length],
  );
}

async function openImportFilePicker(segment) {
  const filePaths = await pickVideoPaths(true);
  if (filePaths.length === 0) return;

  const previousState = getSegmentImportState(segment.id);
  const targetCount = Number(segment.count) || 0;
  const selectedPaths = buildFilledImportPaths(filePaths, targetCount);
  const importedVideos = [];

  try {
    for (const [index, filePath] of selectedPaths.entries()) {
      const assetId = segment.defaultAssets?.[index]?.id || '';
      importedVideos.push(
        await createImportedVideoFromPath(
          filePath,
          `${segment.id}-${index}`,
          assetId,
        ),
      );
    }
  } catch (error) {
    systemMessage.error(error?.message || '素材导入失败');
    return;
  }

  try {
    await deleteReplacedProjectAssets(previousState.videos, importedVideos);
  } catch (error) {
    systemMessage.error(error?.message || '旧素材清理失败');
  }

  previousState.videos.forEach(revokeImportedVideo);
  segmentImportState[segment.id] = {
    imported: importedVideos.length >= targetCount,
    videos: importedVideos,
  };

  if (importedVideos[0]) {
    selectVideoForTimeline(importedVideos[0], segment.name);
  }
}

function requestOneClickImport(segment) {
  if (hasImportedVideoInSegment(segment)) {
    pendingImportSegment.value = segment;
    importOverwriteConfirmVisible.value = true;
    return;
  }

  openImportFilePicker(segment);
}

function closeImportOverwriteConfirm() {
  importOverwriteConfirmVisible.value = false;
  pendingImportSegment.value = null;
}

async function confirmImportOverwrite() {
  const segment = pendingImportSegment.value;
  importOverwriteConfirmVisible.value = false;
  pendingImportSegment.value = null;

  if (segment) {
    await openImportFilePicker(segment);
  }
}

// 替换单个已导入视频，同时清理被替换的工程素材文件。
async function openReplaceFilePicker(segment, videoIndex) {
  const [filePath] = await pickVideoPaths(false);
  const previousState = getSegmentImportState(segment.id);

  if (
    !filePath ||
    videoIndex < 0 ||
    videoIndex >= previousState.videos.length
  ) {
    return;
  }

  const videos = [...previousState.videos];
  const assetId =
    videos[videoIndex]?.assetId ||
    segment.defaultAssets?.[videoIndex]?.id ||
    '';
  let replacementVideo;

  try {
    replacementVideo = await createImportedVideoFromPath(
      filePath,
      `${segment.id}-replace-${videoIndex}`,
      assetId,
    );
  } catch (error) {
    systemMessage.error(error?.message || '素材替换失败');
    return;
  }

  try {
    await deleteReplacedProjectAssets([videos[videoIndex]], [replacementVideo]);
  } catch (error) {
    systemMessage.error(error?.message || '旧素材清理失败');
  }

  revokeImportedVideo(videos[videoIndex]);
  videos[videoIndex] = replacementVideo;

  segmentImportState[segment.id] = {
    imported: previousState.imported,
    videos,
  };

  selectVideoForTimeline(replacementVideo, segment.name);
}

// 选择素材后恢复其时间线状态，播放指针始终从 0 秒开始。
function selectVideoForTimeline(video, styleName = '') {
  cacheCurrentVideoTimelineState();

  const videoInfo =
    typeof video === 'string'
      ? { name: video, duration: selectedVideoDuration.value }
      : video;
  const nextVideoKey = videoInfo.id || videoInfo.name;
  const cachedState = getVideoTimelineState(nextVideoKey);

  selectedVideoName.value = videoInfo.name;
  selectedVideoKey.value = nextVideoKey;
  selectedVideoDuration.value = videoInfo.duration || '00:00';
  selectedVideoSource.value = videoInfo.source || '';
  playerCurrentTime.value = 0;
  timelinePlayheadTime.value = 0;
  timelinePreviewSeeking.value = false;
  playerProgress.value = 0;
  if (
    Number.isFinite(videoInfo.durationSeconds) &&
    videoInfo.durationSeconds > 0
  ) {
    const duration = Math.max(1, videoInfo.durationSeconds);
    timeline.totalDuration = duration;
    timeline.selectedDuration = Math.min(
      timeline.totalDuration,
      getTimelineSelectionDuration(videoInfo, duration),
    );
    const defaultStartTime = Math.max(
      0,
      (timeline.totalDuration - timeline.selectedDuration) / 2,
    );
    timeline.startTime = clampTimelineStart(
      Number.isFinite(cachedState?.startTime)
        ? cachedState.startTime
        : defaultStartTime,
    );
  }
  selectedVideoAssetId.value = videoInfo.assetId || '';
  logTemplateAssetDurationMatch(videoInfo);
  if (styleName) {
    selectedStyleName.value = styleName;
  }
  nextTick(() => {
    resetMainPlayer();
  });
  timelinePulse.value = true;
  window.setTimeout(() => {
    timelinePulse.value = false;
  }, 160);
}

function toggleTimelineContainer() {
  timelineCollapsed.value = !timelineCollapsed.value;
  nextTick(schedulePlayerResize);
}

function clampTimelineStart(startTime) {
  const maxStart = Math.max(
    0,
    timeline.totalDuration - timeline.selectedDuration,
  );
  return Math.min(Math.max(startTime, 0), maxStart);
}

function seekMainPlayerToTimelineTime(targetTime) {
  const video = mainVideoRef.value;
  if (!video) return;

  const duration = Number.isFinite(video.duration) ? video.duration : 0;
  const nextTime = duration ? Math.min(duration, targetTime) : targetTime;
  const currentTime = Number.isFinite(video.currentTime)
    ? video.currentTime
    : 0;
  if (Math.abs(currentTime - nextTime) < 0.03) return;

  video.currentTime = Math.max(0, nextTime);
  updatePlayerControls();
}

function seekMainPlayerByTimelineClientX(clientX) {
  const track = timelineTrackRef.value;
  if (!track) return;

  const rect = track.getBoundingClientRect();
  const ratio = rect.width
    ? Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
    : 0;
  const targetTime = ratio * Math.max(0, Number(timeline.totalDuration) || 0);

  seekMainPlayerToTimelineTime(targetTime);
}

// 播放头拖动实时 seek，结束拖动时移除全局事件监听。
function startTimelinePlayheadDrag(event) {
  event.preventDefault();
  event.stopPropagation();
  event.currentTarget.setPointerCapture?.(event.pointerId);
  timelinePreviewSeeking.value = false;
  timelinePlayheadDragging.value = true;
  seekMainPlayerByTimelineClientX(event.clientX);

  if (timelinePlayheadMoveHandler) {
    window.removeEventListener('pointermove', timelinePlayheadMoveHandler);
  }
  if (timelinePlayheadUpHandler) {
    window.removeEventListener('pointerup', timelinePlayheadUpHandler);
  }

  timelinePlayheadMoveHandler = (moveEvent) => {
    seekMainPlayerByTimelineClientX(moveEvent.clientX);
  };

  timelinePlayheadUpHandler = () => {
    timelinePlayheadDragging.value = false;
    window.removeEventListener('pointermove', timelinePlayheadMoveHandler);
    window.removeEventListener('pointerup', timelinePlayheadUpHandler);
    timelinePlayheadMoveHandler = null;
    timelinePlayheadUpHandler = null;
  };

  window.addEventListener('pointermove', timelinePlayheadMoveHandler);
  window.addEventListener('pointerup', timelinePlayheadUpHandler);
}

// 同一素材可能出现在多个模板区域中，统一计算并提交各区域偏移。
function getTemplateAreaOffsetPayload(assetId, baseOffsetMs) {
  const matches = findTemplateAreaMatches(assetId).filter(
    (match) => match.areaId,
  );
  if (matches.length === 0) return [];

  const maxDurationMs = Math.max(
    ...matches.map((match) => Number(match.durationMs) || 0),
  );
  if (!Number.isFinite(maxDurationMs) || maxDurationMs <= 0) return [];

  return matches.map((match) => {
    const matchDurationMs = Number(match.durationMs);
    const durationMs =
      Number.isFinite(matchDurationMs) && matchDurationMs > 0
        ? matchDurationMs
        : maxDurationMs;
    const centeredOffsetMs = Math.max(0, maxDurationMs - durationMs) / 2;

    return {
      areaId: match.areaId,
      offsetMs: Math.max(0, Math.round(baseOffsetMs + centeredOffsetMs)),
    };
  });
}

async function initializeVideoDefaultOffset(video) {
  const assetId = String(video?.assetId || '').trim();
  if (!activeProjectDir.value || !assetId) return;

  const areaMatches = findTemplateAreaMatches(assetId);
  if (areaMatches.length === 0) return;

  const videoDurationSeconds = Number(video.durationSeconds) || 0;
  const templateDurationSeconds =
    findTemplateAreaDurationSeconds(assetId) || videoDurationSeconds;
  const baseOffsetMs = Math.max(
    0,
    Math.round(((videoDurationSeconds - templateDurationSeconds) * 1000) / 2),
  );
  const areaOffsets = getTemplateAreaOffsetPayload(assetId, baseOffsetMs);

  await invoke('update_project_asset_offset', {
    projectDir: activeProjectDir.value,
    assetId,
    offsetMs: baseOffsetMs,
    areaOffsets,
  });
  videoTimelineStateCache[video.id] = {
    startTime: baseOffsetMs / 1000,
  };
}

// 新工程创建后立即为全部模板视频写入默认居中偏移，避免未操作素材保持 offset=0。
async function initializeProjectAssetOffsets() {
  const initializedAssetIds = new Set();

  for (const segment of importSegments.value) {
    for (const video of segment.videos) {
      const assetId = String(video.assetId || '').trim();
      if (!assetId || initializedAssetIds.has(assetId)) continue;

      initializedAssetIds.add(assetId);
      await initializeVideoDefaultOffset(video);
    }
  }
}

// 暂停状态 seek 后短暂推进一帧，确保 WebView 能刷新视频画面。
function revealPausedVideoFrame(video, targetTime = 0, updateControls = null) {
  if (!video || !video.paused) {
    updateControls?.();
    return;
  }

  const duration = Number.isFinite(video.duration) ? video.duration : 0;
  const normalizedTarget = duration
    ? Math.min(duration, Math.max(0, targetTime))
    : Math.max(0, targetTime);
  const revealTime =
    normalizedTarget > 0
      ? normalizedTarget
      : Math.min(duration || VIDEO_FRAME_REVEAL_TIME, VIDEO_FRAME_REVEAL_TIME);

  const finish = () => {
    video.pause();
    updateControls?.();
  };

  let fallbackTimer = null;
  const onSeeked = () => {
    video.removeEventListener('seeked', onSeeked);
    if (fallbackTimer) {
      window.clearTimeout(fallbackTimer);
      fallbackTimer = null;
    }
    finish();
  };

  video.addEventListener('seeked', onSeeked, { once: true });

  try {
    video.currentTime = revealTime;
    fallbackTimer = window.setTimeout(() => {
      video.removeEventListener('seeked', onSeeked);
      fallbackTimer = null;
      finish();
    }, 300);
  } catch (error) {
    video.removeEventListener('seeked', onSeeked);
    if (fallbackTimer) {
      window.clearTimeout(fallbackTimer);
      fallbackTimer = null;
    }
    finish();
  }
}

function handleMainVideoLoadedMetadata() {
  updatePlayerControls();
  revealPausedVideoFrame(mainVideoRef.value, 0, updatePlayerControls);
}

function handleModalVideoLoadedMetadata() {
  updateModalPreviewControls();
  revealPausedVideoFrame(modalVideoRef.value, 0, updateModalPreviewControls);
}

// 将用户调整后的素材偏移写回工程 XML。
async function persistSelectedVideoOffset(assetId, offsetMs) {
  if (!activeProjectDir.value || !assetId) return;

  try {
    const areaOffsets = getTemplateAreaOffsetPayload(assetId, offsetMs);
    console.log('[duration-track] persist offsets', {
      assetId,
      offsetMs,
      areaOffsets,
    });

    await invoke('update_project_asset_offset', {
      projectDir: activeProjectDir.value,
      assetId,
      offsetMs,
      areaOffsets,
    });
  } catch (error) {
    systemMessage.error(error?.message || '时间偏移更新失败');
  }
}

function scheduleSelectedVideoOffsetPersist() {
  if (offsetPersistTimer) {
    window.clearTimeout(offsetPersistTimer);
  }

  const assetId = selectedVideoAssetId.value;
  const offsetMs = Math.max(0, Math.round(timeline.startTime * 1000));
  if (!activeProjectDir.value || !assetId) return;

  offsetPersistTimer = window.setTimeout(() => {
    offsetPersistTimer = null;
    persistSelectedVideoOffset(assetId, offsetMs);
  }, 300);
}

async function flushSelectedVideoOffsetPersist() {
  if (offsetPersistTimer) {
    window.clearTimeout(offsetPersistTimer);
    offsetPersistTimer = null;
  }

  const assetId = selectedVideoAssetId.value;
  const offsetMs = Math.max(0, Math.round(timeline.startTime * 1000));
  if (!activeProjectDir.value || !assetId) return;

  await persistSelectedVideoOffset(assetId, offsetMs);
}

async function applySubtitleChange() {
  const text = subtitleText.value.trim();
  if (!text) {
    systemMessage.error('请输入内容');
    return;
  }
  if (!activeProjectDir.value) {
    systemMessage.error('请先开始编辑');
    return;
  }

  subtitleApplying.value = true;
  try {
    const projectXml = await invoke('apply_project_subtitle', {
      projectDir: activeProjectDir.value,
      text,
    });
    scheduleProjectTemplateUpdate(projectXml);
    subtitleText.value = text;
    systemMessage.success('标题已更新');
  } catch (error) {
    systemMessage.error(error?.message || '标题更新失败');
  } finally {
    subtitleApplying.value = false;
  }
}

// 拖动选区时限制其始终落在时间线有效范围内。
function startTimelineDrag(event) {
  const track = timelineTrackRef.value;
  if (!track) return;

  const trackRect = track.getBoundingClientRect();
  const trackWidth = track.clientWidth;
  const trackLeft = trackRect.left + track.clientLeft;
  if (!trackWidth) return;

  const selectionRect = event.currentTarget.getBoundingClientRect();
  const offset = event.clientX - selectionRect.left;
  const maxLeft = Math.max(0, trackWidth - selectionRect.width);
  event.currentTarget.setPointerCapture?.(event.pointerId);
  timelinePreviewSeeking.value = true;
  seekMainPlayerToTimelineTime(timeline.startTime);

  if (timelineMoveHandler) {
    window.removeEventListener('pointermove', timelineMoveHandler);
  }
  if (timelineUpHandler) {
    window.removeEventListener('pointerup', timelineUpHandler);
  }

  timelineMoveHandler = (moveEvent) => {
    const rawLeft = moveEvent.clientX - trackLeft - offset;
    const clampedLeft = Math.min(Math.max(rawLeft, 0), Math.max(0, maxLeft));
    const maxStart = Math.max(
      0,
      timeline.totalDuration - timeline.selectedDuration,
    );
    timeline.startTime = maxLeft ? (clampedLeft / maxLeft) * maxStart : 0;
    cacheCurrentVideoTimelineState();
    seekMainPlayerToTimelineTime(timeline.startTime);
  };
  timelineUpHandler = () => {
    timelineDragging.value = false;
    window.removeEventListener('pointermove', timelineMoveHandler);
    window.removeEventListener('pointerup', timelineUpHandler);
    timelineMoveHandler = null;
    timelineUpHandler = null;
    scheduleSelectedVideoOffsetPersist();
  };

  timelineDragging.value = true;
  window.addEventListener('pointermove', timelineMoveHandler);
  window.addEventListener('pointerup', timelineUpHandler);
}

// 工程库浏览、编辑标题和删除操作。
function showDraftLibrary() {
  if (draftLibraryVisible.value) return;

  resetDraftTitleEdit();
  finishedLibraryVisible.value = false;
  draftLibraryVisible.value = true;
  draftFilter.value = 'all';
  loadMyProjects();
}

function hideDraftLibrary() {
  resetDraftTitleEdit();
  goHome();
}

function resetDraftTitleEdit() {
  editingDraftId.value = '';
  draftEditTitle.value = '';
}

function setDraftTitleInputRef(projectId, element) {
  const key = String(projectId || '');
  if (!key) return;

  if (element) {
    draftTitleInputRefs.set(key, element);
  } else {
    draftTitleInputRefs.delete(key);
  }
}

function startDraftTitleEdit(project) {
  if (draftBatchDeleteMode.value) return;

  editingDraftId.value = project.id;
  draftEditTitle.value = project.title;
  nextTick(() => {
    const input = draftTitleInputRefs.get(String(project.id));
    input?.focus();
    input?.select();
  });
}

async function saveDraftTitle(project) {
  if (editingDraftId.value !== project.id || draftRenamingId.value) return;

  const nextTitle = draftEditTitle.value.trim();
  const projectId = getDraftProjectDeleteId(project);
  if (!nextTitle) {
    systemMessage.error('工程名称不能为空');
    resetDraftTitleEdit();
    return;
  }
  if (!projectId) {
    systemMessage.error('工程 ID 不存在');
    resetDraftTitleEdit();
    return;
  }
  if (nextTitle === project.title) {
    resetDraftTitleEdit();
    return;
  }

  draftRenamingId.value = String(projectId);
  try {
    const response = await renameProject({
      projectId,
      projectName: nextTitle,
    });

    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '工程名称修改失败');
    }

    project.title = nextTitle;
    project.projectName = nextTitle;
    systemMessage.success(response?.msg || '工程名称修改成功');
  } catch (error) {
    systemMessage.error(error?.message || '工程名称修改失败');
  } finally {
    draftRenamingId.value = '';
    resetDraftTitleEdit();
  }
}

function getDraftProjectDeleteId(project) {
  return project?.projectId || project?.deleteId || '';
}

function resetDraftBatchDelete() {
  resetDraftTitleEdit();
  draftBatchDeleteMode.value = false;
  draftDeleteConfirmVisible.value = false;
  selectedDraftProjectIds.value = new Set();
}

function toggleDraftBatchDeleteMode() {
  if (!draftBatchDeleteMode.value) {
    resetDraftTitleEdit();
    draftBatchDeleteMode.value = true;
    selectedDraftProjectIds.value = new Set();
    return;
  }

  if (!selectedDraftProjectIds.value.size) {
    systemMessage.error('请选择要删除的工程');
    return;
  }

  draftDeleteConfirmVisible.value = true;
}

function toggleDraftProjectSelection(project) {
  const id = getDraftProjectDeleteId(project);
  if (!id) return;

  const nextSelected = new Set(selectedDraftProjectIds.value);
  const key = String(id);
  if (nextSelected.has(key)) {
    nextSelected.delete(key);
  } else {
    nextSelected.add(key);
  }
  selectedDraftProjectIds.value = nextSelected;
}

function isDraftProjectSelected(project) {
  const id = getDraftProjectDeleteId(project);
  return Boolean(id) && selectedDraftProjectIds.value.has(String(id));
}

function requestSingleDraftDelete(project) {
  const id = getDraftProjectDeleteId(project);
  if (!id) return;

  selectedDraftProjectIds.value = new Set([String(id)]);
  draftDeleteConfirmVisible.value = true;
}

function cancelDraftDeleteConfirm() {
  resetDraftBatchDelete();
}

async function confirmDraftBatchDelete() {
  const ids = Array.from(selectedDraftProjectIds.value);
  if (!ids.length || draftDeleting.value) return;

  draftDeleting.value = true;
  try {
    const response = await deleteProjects({
      ids: ids.join(','),
      pageNum: 1,
      pageSize: 9999,
      renter_id: getStoredTenantId(),
      userId: getStoredUserId(),
    });

    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '工程删除失败');
    }

    try {
      await invoke('delete_project_workspaces', {
        projectIds: ids.map(String),
      });
    } catch (error) {
      console.error('[project] local workspace delete failed:', error);
    }

    resetDraftBatchDelete();
    await loadMyProjects();
    systemMessage.success(response?.msg || '工程删除成功');
  } catch (error) {
    systemMessage.error(error?.message || '工程删除失败');
  } finally {
    draftDeleting.value = false;
  }
}

async function openDraftProject(projectId) {
  if (!projectId || draftOpeningId.value) return;

  draftOpeningId.value = String(projectId);
  try {
    const response = await getProjectDetail({
      projectId: normalizeBackendId(projectId),
    });
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '工程详情查询失败');
    }

    const detail = getResponsePayload(response) || {};
    const detailProjectId = detail.projectId || projectId;
    const templateId = detail.templateId;
    if (!detailProjectId || !templateId) {
      throw new Error('工程详情缺少工程 ID 或模板 ID');
    }

    const workspace = await invoke('read_project_workspace', {
      projectId: String(detailProjectId),
      templateId: String(templateId),
    });
    const templateXml = workspace.templateXml || '';
    const projectFileXml = workspace.projectFileXml || '';
    if (!templateXml || !projectFileXml) {
      throw new Error('本地工程 XML 内容为空');
    }

    clearProjectEditingState();
    activeBackendProjectId.value = detailProjectId;
    activeProjectExported.value =
      String(detail.statusName || '').trim() === '已导出';
    editingFromDraftLibrary.value = true;
    activeTemplateId.value = String(templateId);
    activeTemplateExportCredit.value = Number(detail.exportCredit) || 0;
    activeTemplateName.value = detail.projectName || '未命名工程';
    previewTitle.value = activeTemplateName.value;
    activeProjectDir.value = workspace.projectDir || '';
    activeTemplateLocalInfo.value = {
      templateDir: workspace.projectDir || '',
      templateFilePath: workspace.templateFilePath || '',
      assetsDir: workspace.assetsDir || '',
      xmlContent: templateXml,
      existingAssetIds: workspace.existingAssetIds || [],
    };
    activeTemplateSegments.value = parseTemplateSegments(templateXml);
    activeTemplateDemoSource.value = resolveTemplateVideoSource(
      parseTemplateDemoPath(templateXml),
    );
    subtitleText.value = parseTemplateSubtitleText(templateXml);

    await initializeDefaultTemplateAssets();
    restoreProjectVideoOffsets(projectFileXml);

    draftLibraryVisible.value = false;
    resetDraftBatchDelete();
    sidebarHidden.value = false;
    mainMode.value = 'player';
    timelineCollapsed.value = false;
    currentViewState.value = 'import';
    showFinishedControls.value = false;

    const videos = importSegments.value.flatMap((segment) =>
      segment.videos.map((video) => ({ video, styleName: segment.name })),
    );
    const firstVideo = videos.find(({ video }) => video.source) || videos[0];
    if (firstVideo) {
      selectVideoForTimeline(firstVideo.video, firstVideo.styleName);
    } else {
      selectVideoForTimeline('视频 1', '视频轨道 V1');
    }
    nextTick(schedulePlayerResize);
  } catch (error) {
    systemMessage.error(error?.message || '工程打开失败');
  } finally {
    draftOpeningId.value = '';
  }
}

function showFinishedLibrary() {
  draftLibraryVisible.value = false;
  resetDraftBatchDelete();
  finishedLibraryVisible.value = true;
  sidebarHidden.value = true;
  activeFavoriteFilter.value = 'all';
  loadFavoriteTemplates();
}

function hideFinishedLibrary() {
  goHome();
}

function filterFavorites(filterKey) {
  activeFavoriteFilter.value = filterKey;
}

function openFavoriteTemplate(template) {
  finishedLibraryVisible.value = false;
  openTemplatePreview(template);
}

// 从工程库或成片库打开主播放器。
function openPlayerFromLibrary(displayName) {
  sidebarHidden.value = false;
  mainMode.value = 'player';
  timelineCollapsed.value = false;
  currentViewState.value = 'finished';
  activeTemplateName.value = displayName;
  showFinishedControls.value = true;
  selectVideoForTimeline(displayName, '主线叙事');
  resetMainPlayer();
  nextTick(schedulePlayerResize);
}

// 视频导出流程和进度事件处理。
function resetExportProgress() {
  exportProgress.value = 0;
  exportStatus.value = '正在准备导出...';
  exportOutputPath.value = '';
}

function markProjectExportRecorded() {
  activeProjectExported.value = true;
}

function syncStoredCreditBalance(creditBalance) {
  if (creditBalance === undefined || creditBalance === null) return;

  const userInfo = getStoredUserInfo();
  const nextUserInfo = { ...userInfo, creditBalance };

  for (const key of ['user', 'profile', 'sysUser']) {
    if (userInfo[key] && typeof userInfo[key] === 'object') {
      nextUserInfo[key] = { ...userInfo[key], creditBalance };
    }
  }

  localStorage.setItem('userInfo', JSON.stringify(nextUserInfo));
  userInfoRevision.value += 1;
}

async function ensureProjectExportRecorded(exportPath) {
  const projectId = activeBackendProjectId.value;
  if (!projectId) {
    systemMessage.error('工程 ID 不存在，无法导出');
    return false;
  }

  if (activeProjectExported.value) {
    return true;
  }

  const response = await recordProjectExport({
    exportPath,
    projectId: normalizeBackendId(projectId),
  });

  if (response?.code !== undefined && Number(response.code) !== 0) {
    throw new Error(response?.msg || '导出记录失败');
  }

  const payload = getResponsePayload(response) || {};

  if (Number(payload.result) !== 0) {
    systemMessage.error(payload.resultDesc || response?.msg || '无法导出');
    return false;
  }

  syncStoredCreditBalance(payload.creditBalance);
  markProjectExportRecorded();
  return true;
}

async function showExportConfirmation() {
  console.log('[export] export button clicked');
  if (exportRunning.value) return;
  if (!canExport.value) {
    console.warn(
      '[export] export disabled because editing project is not ready',
    );
    systemMessage.error('请先开始编辑');
    return;
  }
  if (!activeProjectDir.value) {
    console.warn('[export] missing activeProjectDir');
    systemMessage.error('请先开始编辑');
    return;
  }
  if (!activeTemplateLocalInfo.value?.templateFilePath) {
    console.warn('[export] missing templateFilePath');
    systemMessage.error('模板文件不存在');
    return;
  }

  if (hasDefaultTemplateVideos()) {
    defaultTemplateExportConfirmVisible.value = true;
    return;
  }

  await selectExportDirectory();
}

function cancelDefaultTemplateExport() {
  defaultTemplateExportConfirmVisible.value = false;
}

async function confirmDefaultTemplateExport() {
  defaultTemplateExportConfirmVisible.value = false;
  await selectExportDirectory();
}

async function selectExportDirectory() {
  if (exportInterval) clearInterval(exportInterval);
  resetExportProgress();

  try {
    console.log('[export] ensuring default output directory');
    const defaultPath = await invoke('ensure_default_output_dir');
    console.log('[export] default output directory:', defaultPath);
    console.log('[export] opening output directory picker');
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath,
    });

    if (!selected) {
      console.log('[export] output directory selection cancelled');
      return;
    }

    exportSelectedDir.value = Array.isArray(selected) ? selected[0] : selected;
    console.log('[export] selected output directory:', exportSelectedDir.value);
    exportState.value = 'confirm';
    exportModalVisible.value = true;
  } catch (error) {
    console.error('[export] failed to select output directory:', error);
    systemMessage.error(error?.message || '选择导出目录失败');
  }
}

async function openExportDirectory() {
  const outputPath = exportOutputPath.value;
  const outputDir = exportSelectedDir.value;

  console.log('[export] open export directory clicked', {
    outputPath,
    outputDir,
  });

  try {
    if (outputPath) {
      await revealItemInDir(outputPath);
      console.log('[export] reveal output file success:', outputPath);
      return;
    }

    if (outputDir) {
      await openPath(outputDir);
      console.log('[export] open output directory success:', outputDir);
      return;
    }

    systemMessage.error('导出目录不存在');
  } catch (error) {
    console.error('[export] open export directory failed:', error);

    if (outputDir) {
      try {
        await openPath(outputDir);
        console.log(
          '[export] fallback open output directory success:',
          outputDir,
        );
        return;
      } catch (fallbackError) {
        console.error(
          '[export] fallback open output directory failed:',
          fallbackError,
        );
      }
    }

    systemMessage.error(error?.message || '打开导出目录失败');
  }
}

function closeExportModal() {
  if (exportRunning.value) {
    systemMessage.info('导出进行中，请稍候');
    return;
  }

  exportModalVisible.value = false;
  if (exportInterval) {
    clearInterval(exportInterval);
    exportInterval = null;
  }
  resetExportProgress();
}

async function startExportProgress() {
  console.log('[export] confirm export clicked');
  if (exportRunning.value) return;

  exportRunning.value = true;

  const exportId = `composer-export-${Date.now()}`;
  let unlistenProgress = null;

  try {
    console.log('[export] recording project export before compose');
    const exportAllowed = await ensureProjectExportRecorded(
      exportSelectedDir.value,
    );
    if (!exportAllowed) {
      return;
    }

    exportState.value = 'progress';
    resetExportProgress();

    console.log('[export] flushing timeline offset before compose');
    await flushSelectedVideoOffsetPersist();

    console.log('[export] listening composer progress:', exportId);
    unlistenProgress = await listen('composer-export-progress', (event) => {
      const payload = event.payload || {};
      if (payload.exportId !== exportId) return;

      console.log('[export] progress event:', payload);
      exportProgress.value = Math.max(
        0,
        Math.min(100, Number(payload.progress) || 0),
      );
      exportStatus.value = payload.status || exportStatus.value;
    });

    console.log('[export] invoking compose_project_video', {
      exportId,
      templatePath: activeTemplateLocalInfo.value.templateFilePath,
      projectDir: activeProjectDir.value,
      outputDir: exportSelectedDir.value,
    });
    const result = await invoke('compose_project_video', {
      templatePath: activeTemplateLocalInfo.value.templateFilePath,
      projectDir: activeProjectDir.value,
      outputDir: exportSelectedDir.value,
      exportId,
    });

    exportProgress.value = 100;
    exportStatus.value = '导出完成！';
    exportOutputPath.value = result?.outputPath || '';
    console.log('[export] compose success:', result);
    systemMessage.success('视频导出完成');
  } catch (error) {
    exportStatus.value = '导出失败';
    console.error('[export] compose failed:', error);
    systemMessage.error(error?.message || '视频导出失败');
  } finally {
    exportRunning.value = false;
    if (unlistenProgress) {
      console.log('[export] removing progress listener:', exportId);
      unlistenProgress();
    }
  }
}

// 主播放器和模板预览播放器的控制逻辑。
function formatPlayerTime(value) {
  if (!Number.isFinite(value)) return '00:00';
  const minutes = Math.floor(value / 60)
    .toString()
    .padStart(2, '0');
  const seconds = Math.floor(value % 60)
    .toString()
    .padStart(2, '0');
  return `${minutes}:${seconds}`;
}

function formatTimelineTick(value) {
  if (!Number.isFinite(value)) return '0s';
  const seconds = Math.max(0, Math.round(value));

  if (seconds < 60) {
    return `${seconds}s`;
  }

  return formatPlayerTime(seconds);
}

function formatTimelineRulerTick(value) {
  const majorStep = timelineRulerScale.value.majorStep;
  const decimals =
    majorStep >= 1 ? 0 : Math.min(3, Math.ceil(-Math.log10(majorStep)));
  const seconds = Math.max(0, Number(value.toFixed(decimals)));

  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Number((seconds % 60).toFixed(decimals));
  return `${minutes}:${String(remainingSeconds).padStart(2, '0')}`;
}

function updatePlayerControls() {
  const video = mainVideoRef.value;
  if (!video) return;

  const duration = video.duration || 0;
  playerProgress.value = duration
    ? Math.max(0, Math.min(100, (video.currentTime / duration) * 100))
    : 0;
  playerCurrentTime.value = Number.isFinite(video.currentTime)
    ? Math.max(0, video.currentTime)
    : 0;
  if (!timelinePreviewSeeking.value) {
    timelinePlayheadTime.value = playerCurrentTime.value;
  }
  playerTimeLabel.value = `${formatPlayerTime(video.currentTime)} / ${formatPlayerTime(duration)}`;
  playerPaused.value = video.paused;
  playerMuted.value = video.muted || video.volume === 0;
  playerSpeed.value = video.playbackRate;
}

function resetMainPlayer() {
  const video = mainVideoRef.value;
  if (!video) return;
  timelinePreviewSeeking.value = false;
  video.pause();
  video.currentTime = 0;
  updatePlayerControls();
}

function togglePlayerPlayback() {
  const video = mainVideoRef.value;
  if (!video) return;
  timelinePreviewSeeking.value = false;
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
  timelinePreviewSeeking.value = false;
  const duration = video.duration || 0;
  video.currentTime = Math.max(
    0,
    Math.min(duration, video.currentTime + seconds),
  );
  updatePlayerControls();
}

function seekPlayerTo(event) {
  const video = mainVideoRef.value;
  if (!video || !video.duration) return;
  timelinePreviewSeeking.value = false;
  const rect = event.currentTarget.getBoundingClientRect();
  const ratio = (event.clientX - rect.left) / rect.width;
  video.currentTime = Math.max(
    0,
    Math.min(video.duration, ratio * video.duration),
  );
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
  video.playbackRate = modalPlaybackRate.value;
  modalPaused.value = true;
  modalProgress.value = 0;
}

function updateModalPreviewControls() {
  const video = modalVideoRef.value;
  if (!video) return;

  const duration = video.duration || 0;
  modalProgress.value = duration
    ? Math.min(100, Math.max(0, (video.currentTime / duration) * 100))
    : 0;
  modalPaused.value = video.paused;
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
  updateModalPreviewControls();
}

function seekModalPreviewBy(seconds) {
  const video = modalVideoRef.value;
  if (!video) return;

  const duration = Number.isFinite(video.duration) ? video.duration : 0;
  const nextTime = video.currentTime + seconds;
  video.currentTime = duration
    ? Math.max(0, Math.min(duration, nextTime))
    : Math.max(0, nextTime);
  updateModalPreviewControls();
}

function seekModalPreviewTo(event) {
  const video = modalVideoRef.value;
  if (!video || !video.duration) return;

  seekModalPreviewByClientX(event.currentTarget, event.clientX);
}

function seekModalPreviewByClientX(track, clientX) {
  const video = modalVideoRef.value;
  if (!video || !video.duration || !track) return;

  const rect = track.getBoundingClientRect();
  const ratio = (clientX - rect.left) / rect.width;
  video.currentTime = Math.max(
    0,
    Math.min(video.duration, ratio * video.duration),
  );
  updateModalPreviewControls();
}

function startModalPreviewProgressDrag(event) {
  event.preventDefault();
  event.stopPropagation();
  const track = event.currentTarget;
  track.setPointerCapture?.(event.pointerId);
  modalPreviewProgressDragging.value = true;
  seekModalPreviewByClientX(track, event.clientX);

  if (modalPreviewProgressMoveHandler) {
    window.removeEventListener('pointermove', modalPreviewProgressMoveHandler);
  }
  if (modalPreviewProgressUpHandler) {
    window.removeEventListener('pointerup', modalPreviewProgressUpHandler);
  }

  modalPreviewProgressMoveHandler = (moveEvent) => {
    seekModalPreviewByClientX(track, moveEvent.clientX);
  };

  modalPreviewProgressUpHandler = () => {
    modalPreviewProgressDragging.value = false;
    window.removeEventListener('pointermove', modalPreviewProgressMoveHandler);
    window.removeEventListener('pointerup', modalPreviewProgressUpHandler);
    modalPreviewProgressMoveHandler = null;
    modalPreviewProgressUpHandler = null;
  };

  window.addEventListener('pointermove', modalPreviewProgressMoveHandler);
  window.addEventListener('pointerup', modalPreviewProgressUpHandler);
}

function cycleModalPlaybackRate() {
  const speeds = [1, 1.5, 2, 3];
  const currentIndex = speeds.indexOf(modalPlaybackRate.value);
  const rate = speeds[(currentIndex + 1) % speeds.length];
  modalPlaybackRate.value = rate;

  const video = modalVideoRef.value;
  if (video) {
    video.playbackRate = rate;
  }
}

// 根据可用舞台尺寸维持播放器 16:9 比例。
function resizePlayerToStage() {
  const stage = playerStageRef.value;
  const wrapper = playerWrapperRef.value;
  if (!stage || !wrapper || mainMode.value !== 'player') return;

  const rect = stage.getBoundingClientRect();
  const availableWidth = Math.max(160, rect.width - 64);
  const availableHeight = Math.max(90, rect.height - 64);
  const widthFromHeight = availableHeight * (16 / 9);
  const targetWidth = Math.min(availableWidth, widthFromHeight);
  wrapper.style.width = `${targetWidth}px`;
  wrapper.style.height = `${targetWidth * (9 / 16)}px`;
}

function updateTimelineRulerWidth() {
  const width = timelineRulerRef.value?.getBoundingClientRect().width;
  if (Number.isFinite(width) && width > 0) {
    timelineRulerWidth.value = width;
  }

  const trackWidth = timelineTrackRef.value?.clientWidth;
  if (Number.isFinite(trackWidth) && trackWidth > 0) {
    timelineTrackWidth.value = trackWidth;
  }
}

function schedulePlayerResize() {
  if (playerResizeFrame) {
    cancelAnimationFrame(playerResizeFrame);
  }

  playerResizeFrame = requestAnimationFrame(() => {
    resizePlayerToStage();
    updateTimelineRulerWidth();
    requestAnimationFrame(resizePlayerToStage);
  });

  if (playerResizeTimer) {
    window.clearTimeout(playerResizeTimer);
  }
  playerResizeTimer = window.setTimeout(() => {
    resizePlayerToStage();
    updateTimelineRulerWidth();
  }, 340);
}

// 登录缓存、通用格式化和接口响应兼容工具。
function getStoredUserInfo() {
  try {
    return JSON.parse(localStorage.getItem('userInfo') || 'null') || {};
  } catch {
    return {};
  }
}

function getStoredUserProfile() {
  const userInfo = getStoredUserInfo();
  return userInfo.user || userInfo.profile || userInfo.sysUser || userInfo;
}

function formatAccountBalance(value) {
  const balance = Number(value);
  if (!Number.isFinite(balance)) return '--';

  return new Intl.NumberFormat('zh-CN', {
    maximumFractionDigits: 2,
  }).format(balance);
}

function getStoredTenantId() {
  const userInfo = getStoredUserInfo();
  return userInfo.tenantId || userInfo.renterId || '';
}

function getStoredUserId() {
  return getStoredUserInfo().userId || '';
}

function getCategoryId(category) {
  return category?.categoryId || category?.id || '';
}

function getActiveTemplateCategoryId() {
  return activeCategory.value >= 0
    ? getCategoryId(categories.value[activeCategory.value])
    : '';
}

function searchTemplatesNow() {
  const activeCategoryId = getActiveTemplateCategoryId();
  currentViewState.value = 'subtopics';
  mainMode.value = 'grid';
  previewModalVisible.value = false;
  loadTemplatesByCategory(
    activeCategoryId ? categories.value[activeCategory.value] : null,
  );
}

function formatTemplateDuration(duration) {
  const seconds = Number(duration);
  if (!Number.isFinite(seconds)) return '--';

  const totalSeconds = Math.max(0, Math.round(seconds));
  if (totalSeconds < 60) return `${totalSeconds}秒`;

  const minutes = Math.floor(totalSeconds / 60);
  const remainingSeconds = totalSeconds % 60;
  return remainingSeconds
    ? `${minutes}分${remainingSeconds}秒`
    : `${minutes}分`;
}

function formatDurationMsToSeconds(durationMs) {
  const milliseconds = Number(String(durationMs || '').trim());
  if (!Number.isFinite(milliseconds) || milliseconds <= 0) return '';

  const seconds = milliseconds / 1000;
  return Number.isInteger(seconds)
    ? String(seconds)
    : seconds.toFixed(1).replace(/\.0$/, '');
}

function formatMaterialDurationRange(minDuration, maxDuration) {
  const minSeconds = formatDurationMsToSeconds(minDuration);
  const maxSeconds = formatDurationMsToSeconds(maxDuration);

  if (minSeconds && maxSeconds) {
    return minSeconds === maxSeconds
      ? `${minSeconds}秒`
      : `${minSeconds}~${maxSeconds}秒`;
  }

  if (minSeconds) return `≥${minSeconds}秒`;
  if (maxSeconds) return `≤${maxSeconds}秒`;
  return '--';
}

function getResponseList(response) {
  if (Array.isArray(response?.rows)) return response.rows;
  if (Array.isArray(response?.data)) return response.data;
  if (Array.isArray(response?.data?.rows)) return response.data.rows;
  if (Array.isArray(response?.data?.list)) return response.data.list;
  if (Array.isArray(response?.list)) return response.list;
  return Array.isArray(response) ? response : [];
}

function getResponsePayload(response) {
  return response?.data && !Array.isArray(response.data)
    ? response.data
    : response;
}

// 将模板 XML 解析为预览视频、素材段和默认素材信息。
function parseTemplateDemoPath(xmlContent) {
  const xml = new DOMParser().parseFromString(xmlContent, 'text/xml');

  if (!xml.querySelector('parsererror')) {
    return xml.querySelector('demo-path')?.textContent?.trim() || '';
  }

  return getElementText(xmlContent, 'demo-path');
}

function parseTemplateSubtitleText(xmlContent) {
  const xml = new DOMParser().parseFromString(xmlContent, 'text/xml');

  if (!xml.querySelector('parsererror')) {
    const subtitle = xml.querySelector('subtitle');
    return (
      subtitle?.querySelector('default')?.textContent?.trim() ||
      subtitle?.getAttribute('text') ||
      ''
    );
  }

  const subtitleBody = getElementText(xmlContent, 'subtitle');
  return getElementText(subtitleBody, 'default');
}

function parseProjectMaxOffsets(projectFileXml) {
  const maxOffsets = new Map();
  const recordOffset = (assetId, offsetValue) => {
    const offset = Number(offsetValue);
    if (!assetId || !Number.isFinite(offset)) return;
    maxOffsets.set(assetId, Math.max(maxOffsets.get(assetId) || 0, offset));
  };
  const xml = new DOMParser().parseFromString(projectFileXml, 'text/xml');

  if (!xml.querySelector('parsererror')) {
    xml.querySelectorAll('area').forEach((area) => {
      recordOffset(
        area.getAttribute('asset-id') || '',
        area.getAttribute('offset') || 0,
      );
    });
    return maxOffsets;
  }

  for (const match of projectFileXml.matchAll(/<area\b([^>]*)\/?>/gi)) {
    recordOffset(
      getAttributeValue(match[1] || '', 'asset-id'),
      getAttributeValue(match[1] || '', 'offset'),
    );
  }

  return maxOffsets;
}

function parseTemplateSegments(xmlContent) {
  const xml = new DOMParser().parseFromString(xmlContent, 'text/xml');

  if (xml.querySelector('parsererror')) {
    return parseTemplateSegmentsFromText(xmlContent);
  }

  return Array.from(xml.querySelectorAll('media-asset')).map(
    (mediaAsset, index) => {
      const directChildren = Array.from(mediaAsset.children);
      const defaultAsset = directChildren.find(
        (child) => child.tagName.toLowerCase() === 'default-asset',
      );
      const constraints = directChildren.find(
        (child) => child.tagName.toLowerCase() === 'constraints',
      );
      const constraintChildren = constraints
        ? Array.from(constraints.children)
        : [];
      const minDuration =
        constraintChildren
          .find((child) => child.tagName.toLowerCase() === 'minduration')
          ?.textContent?.trim() || '';
      const maxDuration =
        constraintChildren
          .find((child) => child.tagName.toLowerCase() === 'maxduration')
          ?.textContent?.trim() || '';
      const defaultAssets = defaultAsset
        ? Array.from(defaultAsset.children).filter(
            (child) => child.tagName.toLowerCase() === 'asset',
          )
        : [];
      const defaultAssetItems = defaultAssets.map((asset) => ({
        id: asset.getAttribute('id') || '',
        filepath: asset.getAttribute('filepath') || '',
      }));
      const comment =
        directChildren
          .find((child) => child.tagName.toLowerCase() === 'comment')
          ?.textContent?.trim() || '';
      const name =
        mediaAsset.getAttribute('name') ||
        directChildren
          .find((child) => child.tagName.toLowerCase() === 'name')
          ?.textContent?.trim() ||
        mediaAsset.getAttribute('id') ||
        `素材 ${index + 1}`;

      return {
        name,
        shot: comment,
        count: defaultAssets.length,
        durationRange: formatMaterialDurationRange(minDuration, maxDuration),
        defaultAssets: defaultAssetItems,
      };
    },
  );
}

function getElementText(source, name) {
  const match = source.match(
    new RegExp(`<${name}\\b[^>]*>([\\s\\S]*?)<\\/${name}>`, 'i'),
  );
  return match?.[1]?.trim() || '';
}

function getAttributeValue(source, name) {
  const quotedMatch = source.match(
    new RegExp(`${name}\\s*=\\s*["']([^"']*)["']`, 'i'),
  );
  if (quotedMatch?.[1]) {
    return quotedMatch[1].trim();
  }

  const unquotedMatch = source.match(
    new RegExp(`${name}\\s*=\\s*([^\\s>]+)`, 'i'),
  );
  return unquotedMatch?.[1]?.trim() || '';
}

function parseAssetTags(source) {
  return Array.from(source.matchAll(/<asset\b([^>]*)\/?>/gi)).map((match) => {
    const attributes = match[1] || '';

    return {
      id: getAttributeValue(attributes, 'id'),
      filepath: getAttributeValue(attributes, 'filepath'),
    };
  });
}

function parseTemplateSegmentsFromText(xmlContent) {
  const mediaAssetMatches = Array.from(
    xmlContent.matchAll(/<media-asset\b([^>]*)>([\s\S]*?)<\/media-asset>/gi),
  );

  return mediaAssetMatches.map((match, index) => {
    const attributes = match[1] || '';
    const body = match[2] || '';
    const defaultAssetBody = getElementText(body, 'default-asset');
    const name =
      getAttributeValue(attributes, 'name') ||
      getElementText(body, 'name') ||
      getAttributeValue(attributes, 'id') ||
      `素材 ${index + 1}`;

    return {
      name,
      shot: getElementText(body, 'comment'),
      count: (defaultAssetBody.match(/<asset\b/gi) || []).length,
      durationRange: formatMaterialDurationRange(
        getElementText(body, 'minDuration'),
        getElementText(body, 'maxDuration'),
      ),
      defaultAssets: parseAssetTags(defaultAssetBody),
    };
  });
}

// 将后端模板和工程数据转换为页面统一展示模型。
function mapTemplateTopic(template, index) {
  const clipCount =
    Number(template.mediaCount) ||
    Number(template.clipCount) ||
    Number(template.assetsCount) ||
    0;
  const title = template.templateName || template.title || `模板 ${index + 1}`;
  const duration = formatTemplateDuration(
    template.videoDuration ?? template.duration,
  );
  const materialTypeCount = Number(template.assetsCount) || 0;
  const favorited = Number(template.favorite) === 1;

  return {
    ...template,
    id: template.templateId || template.id || `template-${index}`,
    favorite: favorited ? 1 : 0,
    favorited,
    title,
    count: clipCount,
    subtitle: `${clipCount}个片段 · ${title}`,
    materialTypeCount,
    material: clipCount,
    duration,
    meta: `${materialTypeCount}类 · ${clipCount}个素材 · ${duration}`,
    image: template.coverPic || template.thumbnailUrl || template.image || '',
  };
}

function normalizeProjectStatus(status) {
  return status === 1 ||
    status === '1' ||
    status === '已导出' ||
    status === 'exported'
    ? 'exported'
    : 'editing';
}

function mapProject(project, index) {
  const status = normalizeProjectStatus(project.status);
  const statusLabel =
    project.statusName || (status === 'exported' ? '已导出' : '编辑中');

  return {
    ...project,
    id: project.projectId || project.id || `project-${index}`,
    deleteId: project.projectId || project.id || '',
    title: project.projectName || project.title || '未命名工程',
    status,
    duration: project.duration || '--:--',
    time: statusLabel,
    image: project.coverPic || project.thumbnailUrl || project.image || '',
  };
}

// 模板、分类、推荐和工程库接口加载。
async function loadTemplatesByCategory(category) {
  const requestId = ++templateRequestId;
  const recommendationRequest = ++recommendationRequestId;
  templatesLoading.value = true;
  recommendationsLoading.value = true;

  try {
    const topics = await fetchTemplateTopics(
      getCategoryId(category),
      templateSearchKeyword.value,
    );

    if (requestId !== templateRequestId) return;

    subtopics.value = topics;
    if (recommendationRequest === recommendationRequestId) {
      recommendationCards.value = topics;
    }
  } catch (error) {
    if (requestId === templateRequestId) {
      subtopics.value = [];
      if (recommendationRequest === recommendationRequestId) {
        recommendationCards.value = [];
      }
      systemMessage.error(error?.message || '模板加载失败');
    }
  } finally {
    if (requestId === templateRequestId) {
      templatesLoading.value = false;
    }
    if (recommendationRequest === recommendationRequestId) {
      recommendationsLoading.value = false;
    }
  }
}

async function fetchTemplateTopics(categoryId = '', keyword = '') {
  const payload = {
    favorite: '',
    keyword: String(keyword || '').trim(),
    pageNum: 1,
    pageSize: 9999,
    renter_id: getStoredTenantId(),
    sortType: '',
    templateCategoryId: categoryId || '',
    userId: getStoredUserId(),
  };

  const response = await getTemplates(payload);
  const topics = getResponseList(response).map(mapTemplateTopic);
  syncTemplateFavoriteStates(topics);

  return topics;
}

async function loadRecommendedTemplates() {
  const requestId = ++recommendationRequestId;
  recommendationsLoading.value = true;

  try {
    const topics = await fetchTemplateTopics('');
    if (requestId === recommendationRequestId) {
      recommendationCards.value = topics;
    }
  } catch (error) {
    if (requestId === recommendationRequestId) {
      recommendationCards.value = [];
      systemMessage.error(error?.message || '推荐模板加载失败');
    }
  } finally {
    if (requestId === recommendationRequestId) {
      recommendationsLoading.value = false;
    }
  }
}

async function loadTemplateCategories() {
  try {
    const renterId = getStoredTenantId();
    const response = await getTemplateCategories(
      renterId ? { renter_id: renterId } : {},
    );
    const list = getResponseList(response);

    categories.value = list;
    activeCategory.value = -1;
    subtopics.value = [];
  } catch (error) {
    systemMessage.error(error?.message || '模板分类加载失败');
  }
}

async function loadMyProjects() {
  try {
    const renterId = getStoredTenantId();
    const userId = getStoredUserId();

    if (!renterId || !userId) {
      draftProjects.value = [];
      return;
    }

    const response = await getMyProjects({
      renter_id: renterId,
      userId: normalizeBackendId(userId),
      pageNum: 1,
      pageSize: 9999,
    });
    const list = getResponseList(response);

    draftProjects.value = list.map(mapProject);
  } catch (error) {
    systemMessage.error(error?.message || '工程库加载失败');
  }
}

// 账户菜单、密码修改、帮助中心和退出登录。
function toggleAccountMenu() {
  accountMenuVisible.value = !accountMenuVisible.value;
}

function closeAccountMenu() {
  accountMenuVisible.value = false;
}

function showProfileModal() {
  profileModalVisible.value = true;
  closeAccountMenu();
}

function hideProfileModal() {
  profileModalVisible.value = false;
}

function resetPasswordForm() {
  passwordForm.oldPassword = '';
  passwordForm.newPassword = '';
  passwordForm.confirmPassword = '';
}

function showPasswordModal() {
  passwordModalVisible.value = true;
  closeAccountMenu();
}

function hidePasswordModal() {
  if (passwordSubmitting.value) return;

  passwordModalVisible.value = false;
  resetPasswordForm();
}

async function submitPasswordReset() {
  if (passwordSubmitting.value) return;

  const oldPassword = passwordForm.oldPassword.trim();
  const newPassword = passwordForm.newPassword.trim();
  const confirmPassword = passwordForm.confirmPassword.trim();

  if (!oldPassword) {
    systemMessage.error('请输入旧密码');
    return;
  }

  if (!newPassword) {
    systemMessage.error('请输入新密码');
    return;
  }

  if (!confirmPassword) {
    systemMessage.error('请确认新密码');
    return;
  }

  if (newPassword !== confirmPassword) {
    systemMessage.error('两次输入的新密码不一致');
    return;
  }

  const profile = getStoredUserProfile();
  const renterId = getStoredTenantId();
  const userId = getStoredUserId();
  const phone = profile.phone || profile.phonenumber || '';

  if (!renterId || !userId || !phone) {
    systemMessage.error('用户信息不完整，无法修改密码');
    return;
  }

  passwordSubmitting.value = true;
  try {
    const response = await resetPassword({
      renterId,
      userId: normalizeBackendId(userId),
      phone,
      modifyType: 0,
      oldPassword,
      newPassword,
    });

    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '修改密码失败');
    }

    systemMessage.success(response?.msg || '修改密码成功，请重新登录');
    passwordModalVisible.value = false;
    resetPasswordForm();
    emit('logout');
  } catch (error) {
    systemMessage.error(error?.message || '修改密码失败');
  } finally {
    passwordSubmitting.value = false;
  }
}

function showHelpCenter() {
  helpCenterVisible.value = true;
  closeAccountMenu();
}

function hideHelpCenter() {
  helpCenterVisible.value = false;
}

function switchHelpTab(tab) {
  activeHelpTab.value = tab;
}

function downloadHelpGuide() {
  systemMessage.info('离线指南暂未开放下载');
}

function showLogoutConfirm() {
  logoutConfirmVisible.value = true;
  closeAccountMenu();
}

function hideLogoutConfirm() {
  if (logoutSubmitting.value) return;

  logoutConfirmVisible.value = false;
}

async function confirmLogout() {
  if (logoutSubmitting.value) return;

  logoutSubmitting.value = true;
  try {
    const response = await logoutUser();
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '退出登录失败');
    }

    logoutConfirmVisible.value = false;
    emit('logout');
  } catch (error) {
    systemMessage.error(error?.message || '退出登录失败');
  } finally {
    logoutSubmitting.value = false;
  }
}

// 点击页面空白处时关闭临时浮层。
function handleWorkspaceClick() {
  closeAccountMenu();
}

// 初始化页面数据、窗口监听和播放器尺寸观察。
onMounted(() => {
  document.title = '艾咔 · AICut - 视频快速剪辑软件';
  document.documentElement.classList.add('dark');
  window.addEventListener('resize', schedulePlayerResize);
  if (window.ResizeObserver) {
    playerResizeObserver = new ResizeObserver(schedulePlayerResize);
    nextTick(() => {
      if (playerStageRef.value) {
        playerResizeObserver.observe(playerStageRef.value);
      }
    });
  }
  loadTemplateCategories();
  loadRecommendedTemplates();
  loadMyProjects();
  nextTick(() => {
    updatePlayerControls();
    schedulePlayerResize();
  });
});

// 离开页面时释放全局监听、定时器和本地视频 URL。
onBeforeUnmount(() => {
  cacheCurrentVideoTimelineState();
  document.documentElement.classList.remove('dark');
  window.removeEventListener('resize', schedulePlayerResize);
  if (exportInterval) {
    clearInterval(exportInterval);
  }
  if (timelineMoveHandler) {
    window.removeEventListener('pointermove', timelineMoveHandler);
  }
  if (timelineUpHandler) {
    window.removeEventListener('pointerup', timelineUpHandler);
  }
  if (timelinePlayheadMoveHandler) {
    window.removeEventListener('pointermove', timelinePlayheadMoveHandler);
  }
  if (timelinePlayheadUpHandler) {
    window.removeEventListener('pointerup', timelinePlayheadUpHandler);
  }
  if (modalPreviewProgressMoveHandler) {
    window.removeEventListener('pointermove', modalPreviewProgressMoveHandler);
  }
  if (modalPreviewProgressUpHandler) {
    window.removeEventListener('pointerup', modalPreviewProgressUpHandler);
  }
  if (playerResizeFrame) {
    cancelAnimationFrame(playerResizeFrame);
  }
  if (playerResizeTimer) {
    window.clearTimeout(playerResizeTimer);
  }
  if (offsetPersistTimer) {
    window.clearTimeout(offsetPersistTimer);
  }
  if (projectUpdateTimer) {
    window.clearTimeout(projectUpdateTimer);
  }
  pendingProjectUpdate = null;
  playerResizeObserver?.disconnect();
  importedVideoObjectUrls.forEach((objectUrl) => {
    URL.revokeObjectURL(objectUrl);
  });
  importedVideoObjectUrls.clear();
});
</script>

<template>
  <div
    class="workspace-page bg-background text-on-surface font-body-md text-body-md selection:bg-primary/30"
    @click="handleWorkspaceClick"
  >
    <header
      class="fixed top-0 left-0 right-0 z-[300] flex flex-col bg-surface-container/80 backdrop-blur-2xl border-b border-primary/20"
    >
      <div
        class="flex items-center px-6 gap-4 overflow-visible no-scrollbar border-b-2 border-white/10 h-16"
      >
        <div class="flex items-center gap-4 shrink-0">
          <img
            alt="艾咔"
            class="h-10 w-auto object-contain cursor-pointer"
            :src="logoImage"
            @click.stop="goHome"
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
          class="h-9 w-24 text-on-surface-variant hover:text-electric-blue shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 shadow-sm rounded-lg transition-all active:scale-95 hover:bg-surface-container-high border border-outline-variant/20"
          @click="showDraftLibrary"
        >
          <span class="text-[13px] font-bold whitespace-nowrap">工程库</span>
        </button>
        <button
          class="h-9 w-24 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant hover:text-electric-blue rounded-lg font-bold shadow-sm hover:bg-surface-container-high active:scale-95 transition-all shrink-0 border border-outline-variant/20"
          :class="{
            'opacity-45 cursor-not-allowed hover:text-on-surface-variant hover:bg-surface-container-low/50 active:scale-100':
              !canExport || exportRunning,
          }"
          type="button"
          :disabled="!canExport || exportRunning"
          @click="showExportConfirmation"
        >
          <span class="text-[13px] uppercase tracking-wide whitespace-nowrap"
            >导出</span
          >
        </button>
        <div class="relative z-[130]" @click.stop>
          <button
            class="h-9 w-24 shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant shadow-sm hover:bg-surface-container-high hover:text-electric-blue focus:bg-electric-blue focus:text-white rounded-lg transition-all focus:outline-none active:scale-95 border border-outline-variant/20"
            type="button"
            @click="toggleAccountMenu"
          >
            <span class="text-[13px] font-bold">个人中心</span>
          </button>
          <div
            class="absolute top-full right-0 mt-2 w-36 bg-surface-container-highest/95 backdrop-blur-xl border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.5)] transition-all duration-200 ease-out z-[110] py-2 overflow-hidden"
            :class="
              accountMenuVisible
                ? 'opacity-100 translate-y-0 pointer-events-auto'
                : 'opacity-0 translate-y-2 pointer-events-none'
            "
          >
            <button
              class="w-full flex items-center gap-3 px-4 py-2.5 text-[13px] text-on-surface-variant hover:bg-electric-blue/10 hover:text-white transition-colors text-left"
              type="button"
              @click="showProfileModal"
            >
              <span class="material-symbols-outlined text-[18px]"
                >account_circle</span
              >
              <span>个人信息</span>
            </button>
            <div class="mx-2 my-1 border-t border-outline-variant/30"></div>
            <button
              class="w-full flex items-center gap-3 px-4 py-2.5 text-[13px] text-on-surface-variant hover:bg-electric-blue/10 hover:text-white transition-colors text-left"
              type="button"
              @click="showPasswordModal"
            >
              <span class="material-symbols-outlined text-[18px]"
                >lock_reset</span
              >
              <span>修改密码</span>
            </button>
            <div class="mx-2 my-1 border-t border-outline-variant/30"></div>
            <button
              class="w-full flex items-center gap-3 px-4 py-2.5 text-[13px] text-on-surface-variant hover:bg-electric-blue/10 hover:text-white transition-colors text-left"
              type="button"
              @click="showHelpCenter"
            >
              <span class="material-symbols-outlined text-[18px]"
                >help_center</span
              >
              <span>帮助中心</span>
            </button>
            <div class="mx-2 my-1 border-t border-outline-variant/30"></div>
            <a
              class="flex items-center gap-3 px-4 py-2.5 text-[13px] text-[#ec4034] hover:bg-[#ec4034]/10 transition-colors"
              href="#"
              @click.prevent="showLogoutConfirm"
              ><span class="material-symbols-outlined text-[18px]">logout</span
              ><span>退出登录</span></a
            >
          </div>
        </div>
      </div>

      <div
        class="h-14 category-toolbar border-b border-primary/10 flex items-center px-6 gap-4 overflow-x-auto no-scrollbar"
      >
        <span
          class="text-on-surface-variant font-label-caps text-[16px] uppercase shrink-0 whitespace-nowrap"
          >模板主题</span
        >
        <div class="category-container flex items-center gap-2 shrink-0">
          <button
            v-for="(category, index) in categories"
            :key="category.categoryId || category.categoryName || index"
            class="category-btn flex items-center gap-2 px-4 py-1.5 rounded-full border text-[12px] font-bold whitespace-nowrap transition-all duration-300"
            :class="
              activeCategory === index
                ? 'bg-electric-blue text-white border-white active-glow'
                : 'border-outline-variant/30 bg-white/5 text-on-surface-variant hover:border-electric-blue/50 hover:text-electric-blue'
            "
            @click="selectCategory(index)"
          >
            <span>{{ category.categoryName || category.categoryId }}</span>
          </button>
        </div>
        <div class="flex-1"></div>
        <button
          class="category-btn flex items-center gap-2 px-4 py-1.5 rounded-full border text-[12px] font-bold whitespace-nowrap transition-all duration-300 shrink-0 border-outline-variant/30 bg-white/5 text-on-surface-variant hover:border-electric-blue/50 hover:text-electric-blue"
          type="button"
          @click="showFinishedLibrary"
        >
          <span>收藏夹</span>
        </button>
        <div class="relative max-w-[200px] shrink-0">
          <span
            class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[16px] text-on-surface-variant/50"
            >search</span
          >
          <input
            v-model="templateSearchKeyword"
            class="w-full bg-surface-container-lowest/50 border border-outline-variant/30 rounded-full pl-9 pr-4 py-1.5 text-[11px] focus:border-electric-blue/50 focus:bg-surface-container-lowest outline-none transition-all text-on-surface placeholder:text-on-surface-variant/40"
            placeholder="搜索主题..."
            type="text"
            @keydown.enter.prevent="searchTemplatesNow"
          />
        </div>
      </div>
    </header>

    <main class="h-[calc(100vh-120px)] flex flex-col mt-[120px]">
      <div class="flex-1 flex overflow-hidden relative">
        <aside
          class="w-72 flex flex-col z-[150] h-full shrink-0 relative shadow-2xl"
          :class="{ 'hidden-sidebar': sidebarHidden }"
        >
          <div
            class="p-4 border-b border-white/5 flex justify-between items-center bg-white/5"
          >
            <h2
              class="font-header-section text-body-md font-bold text-on-surface flex items-center gap-2 whitespace-nowrap flex-1 overflow-hidden"
            >
              <div class="flex flex-col overflow-hidden">
                <span
                  v-if="currentViewState === 'import'"
                  class="text-[17px] text-on-surface tracking-normal truncate"
                  >{{ activeTemplateName }}</span
                >
                <span
                  v-else
                  class="text-[12px] text-on-surface-variant/60 uppercase tracking-[0.18em]"
                  >{{ sidebarContextLabel }}</span
                >
                <span
                  class="tracking-[0.18em] truncate"
                  :class="
                    currentViewState === 'import'
                      ? 'text-[14px] text-on-surface-variant/80'
                      : 'text-[17px]'
                  "
                  >{{ sidebarTitle }}</span
                >
              </div>
              <div class="flex-1"></div>
              <button
                v-if="
                  currentViewState === 'segments' ||
                  (currentViewState === 'import' && !editingFromDraftLibrary)
                "
                class="flex items-center gap-1 px-2.5 py-1.5 rounded-md bg-electric-blue text-[12px] text-white font-bold shrink-0 hover:brightness-110 active:scale-95 transition-all"
                @click="handleSidebarBack"
              >
                <span class="text-[11px]">返回</span>
                <span class="material-symbols-outlined text-[18px]"
                  >arrow_forward</span
                >
              </button>
            </h2>
          </div>
          <div class="flex-1 overflow-y-auto custom-scrollbar flex flex-col">
            <div v-if="currentViewState === 'subtopics'" class="p-4 space-y-4">
              <h3
                v-if="selectedTemplateThemeName"
                class="text-[13px] font-bold text-on-surface-variant uppercase tracking-widest whitespace-nowrap mb-2"
              >
                {{ selectedTemplateThemeName }} > 模版名称
              </h3>
              <div
                v-if="templatesLoading"
                class="px-2 py-6 text-center text-[12px] text-on-surface-variant/70"
              >
                模板加载中...
              </div>
              <div
                v-else-if="
                  !selectedTemplateThemeName && !hasTemplateSearchKeyword
                "
                class="px-2 py-6 text-center text-[12px] text-on-surface-variant/70"
              >
                请选择模板主题
              </div>
              <div
                v-else-if="subtopics.length === 0"
                class="px-2 py-6 text-center text-[12px] text-on-surface-variant/70"
              >
                暂无模板
              </div>
              <div
                v-else
                class="subtopic-selector-grid grid grid-cols-1 gap-2 templateWrapper"
              >
                <button
                  v-for="topic in subtopics"
                  :key="topic.id"
                  class="px-3 py-2.5 rounded bg-surface-container border border-outline-variant text-on-surface-variant hover:border-electric-blue/40 hover:bg-surface-container-high transition-all flex flex-col gap-1.5 text-left group overflow-hidden"
                  :disabled="templateDetailLoading"
                  @click="openTemplatePreview(topic)"
                >
                  <div class="flex items-center gap-2">
                    <div class="text-[13px] font-bold text-on-surface truncate">
                      {{ topic.title }} ({{ topic.count }})
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <span class="text-[11px] opacity-70"
                      >类数: {{ topic.materialTypeCount }}</span
                    >
                    <span class="text-[11px] opacity-70"
                      >素材数: {{ topic.material }}</span
                    >
                    <span class="text-[11px] opacity-70"
                      >时长: {{ topic.duration }}</span
                    >
                  </div>
                </button>
              </div>
            </div>
            <div
              v-else-if="currentViewState === 'segments'"
              class="p-4 space-y-4 flex flex-col h-full"
            >
              <div class="flex items-center justify-between mb-2 shrink-0">
                <h3
                  class="text-[10px] font-bold text-on-surface-variant uppercase tracking-widest"
                >
                  模板素材详情
                </h3>
              </div>
              <div
                class="flex-1 overflow-y-auto custom-scrollbar space-y-3 pt-1 templateDetailWrapper"
              >
                <div
                  v-for="segment in visibleSegments"
                  :key="segment.name"
                  class="segment-card p-3 rounded bg-surface-container-high/50 border border-white/5 hover:border-electric-blue/40 transition-all transform hover:-translate-y-0.5 cursor-pointer shadow-sm"
                >
                  <div class="text-[12px] font-bold text-on-surface">
                    {{ segment.name }}
                  </div>
                  <div class="flex flex-col gap-1 mt-1.5">
                    <div class="flex justify-between text-[10px]">
                      <span class="text-on-surface-variant">所需素材:</span>
                      <span class="text-electric-blue font-bold"
                        >{{ segment.count }} 个视频</span
                      >
                    </div>
                    <div class="flex justify-between text-[10px]">
                      <span class="text-on-surface-variant">素材时长:</span>
                      <span class="text-electric-blue font-bold">{{
                        segment.durationRange || '--'
                      }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="p-4 space-y-4 flex flex-col h-full">
              <div
                class="flex-1 overflow-y-auto custom-scrollbar space-y-3 p-1"
              >
                <template v-if="currentViewState === 'finished'">
                  <div
                    class="px-3 py-8 text-center text-[12px] text-on-surface-variant/70"
                  >
                    暂无视频
                  </div>
                </template>
                <template v-else>
                  <div
                    v-for="style in importSegments"
                    :key="style.id"
                    class="videoList rounded-lg bg-surface-container-low border border-white/5 overflow-hidden mb-3 shadow-sm"
                  >
                    <div
                      class="flex items-center justify-between gap-3 p-3 bg-white/5"
                    >
                      <div class="min-w-0 flex-1">
                        <div class="text-[13px] font-bold text-white truncate">
                          {{ style.name }}
                        </div>
                      </div>
                      <button
                        class="px-3 py-1.5 text-[11px] font-bold rounded-md flex items-center gap-1 shrink-0 transition-colors"
                        :class="
                          isSegmentFullyImported(style)
                            ? 'bg-white/5 text-on-surface-variant/60 border border-white/10 hover:bg-white/10'
                            : 'bg-electric-blue text-white shadow-lg shadow-electric-blue/10 hover:brightness-110'
                        "
                        type="button"
                        @click="requestOneClickImport(style)"
                      >
                        {{ `一键导入 (${style.count})` }}
                      </button>
                    </div>
                    <div
                      v-if="style.videos.length"
                      class="p-3 space-y-2 border-t border-outline-variant"
                    >
                      <div
                        v-for="(video, videoIndex) in style.videos"
                        :key="
                          video.id || `${style.id}-${videoIndex}-${video.name}`
                        "
                        class="flex items-center justify-between p-2 rounded bg-surface-container-lowest/50 border border-white/5 hover:border-electric-blue/40 transition-all cursor-pointer group"
                        :class="{
                          'is-selected':
                            selectedVideoKey ===
                            (video.id ||
                              `${style.id}-${videoIndex}-${video.name}`),
                        }"
                        @click="selectVideoForTimeline(video, style.name)"
                      >
                        <div class="flex items-center gap-2 min-w-0">
                          <span
                            class="material-symbols-outlined text-primary text-[18px] shrink-0"
                            >smart_display</span
                          >
                          <div class="min-w-0">
                            <div class="text-[12px] text-white truncate">
                              {{ video.name }}
                            </div>
                            <div class="text-[10px] text-on-surface-variant">
                              时长：{{ video.duration }}
                            </div>
                          </div>
                        </div>
                        <button
                          type="button"
                          class="px-2 py-1 rounded text-[10px] font-bold shrink-0 transition-colors"
                          :class="
                            isProjectImportedVideo(video)
                              ? 'bg-white/5 text-on-surface-variant/60 border border-white/10 hover:bg-white/10'
                              : 'bg-electric-blue/10 text-electric-blue hover:bg-electric-blue/20'
                          "
                          @click.stop="openReplaceFilePicker(style, videoIndex)"
                        >
                          替换
                        </button>
                      </div>
                    </div>
                  </div>
                </template>
              </div>
            </div>
          </div>
        </aside>

        <section
          class="flex-1 bg-surface-dim flex flex-col h-full overflow-hidden relative"
        >
          <button
            v-if="sidebarToggleVisible"
            class="absolute top-1/2 left-0 -translate-y-1/2 z-[80] bg-surface-container-high border border-outline-variant text-white w-6 h-12 rounded-r-full flex items-center justify-center shadow-lg hover:bg-surface-container-highest transition-all duration-300 group"
            @click="toggleSidebar()"
          >
            <span
              class="material-symbols-outlined text-[18px] group-hover:scale-110"
              >{{ sidebarHidden ? 'chevron_right' : 'chevron_left' }}</span
            >
          </button>
          <div
            class="absolute inset-0 z-[190] bg-black/60 backdrop-blur-[2px] modal-fade-in"
            :class="{ hidden: !previewModalVisible }"
            @click="hidePreviewModal"
          ></div>

          <div
            v-show="mainMode === 'grid'"
            class="w-full h-full flex flex-col relative z-0"
          >
            <div class="p-6 shrink-0">
              <h2
                class="text-[18px] font-black text-on-surface flex items-center gap-2"
              >
                为您推荐
              </h2>
            </div>
            <div class="flex-1 overflow-y-auto custom-scrollbar p-6 pt-0">
              <div
                v-if="recommendationsLoading"
                class="py-10 text-center text-[12px] text-on-surface-variant/70"
              >
                推荐模板加载中...
              </div>
              <div
                v-else-if="recommendationCards.length === 0"
                class="py-10 text-center text-[12px] text-on-surface-variant/70"
              >
                暂无推荐模板
              </div>
              <div
                v-else
                class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6"
              >
                <div
                  v-for="card in recommendationCards"
                  :key="card.id"
                  class="group relative aspect-[16/9] bg-surface-container-high rounded-xl overflow-hidden border border-white/5 hover:border-electric-blue/60 transition-all cursor-pointer shadow-lg hover:shadow-electric-blue/10"
                  @click="openTemplatePreview(card)"
                >
                  <button
                    class="absolute top-2 right-2 z-20 flex h-8 w-8 items-center justify-center rounded-full bg-black/40 backdrop-blur-md transition-colors"
                    :class="
                      isTemplateFavorited(card)
                        ? 'text-yellow-400 hover:text-yellow-300'
                        : 'text-white hover:text-white'
                    "
                    type="button"
                    @click.stop="toggleTemplateFavorite(card)"
                  >
                    <span
                      class="material-symbols-outlined text-[21px]"
                      :class="{
                        'start-editing-spinner':
                          isTemplateFavoriteUpdating(card),
                      }"
                      :style="
                        isTemplateFavorited(card)
                          ? { fontVariationSettings: `'FILL' 1` }
                          : undefined
                      "
                      >{{
                        isTemplateFavoriteUpdating(card)
                          ? 'progress_activity'
                          : 'star'
                      }}</span
                    >
                  </button>
                  <img
                    v-if="card.image"
                    :alt="card.title"
                    class="w-full h-full object-cover opacity-70 group-hover:scale-105 transition-transform duration-500"
                    :src="card.image"
                  />
                  <div class="absolute inset-0 flex flex-col justify-end p-4">
                    <div class="text-[14px] font-black text-white">
                      {{ card.title }}
                    </div>
                    <div class="text-[10px] text-white/60">{{ card.meta }}</div>
                  </div>
                  <div
                    v-if="card.badge"
                    class="absolute top-2 right-11 bg-electric-blue text-white text-[9px] font-black px-2 py-0.5 rounded"
                  >
                    {{ card.badge }}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            v-show="mainMode === 'player'"
            class="w-full flex-1 min-h-0 flex flex-col"
          >
            <div
              ref="playerStageRef"
              class="flex-1 bg-black/90 flex flex-col items-center justify-center relative overflow-hidden p-8"
            >
              <div
                ref="playerWrapperRef"
                class="playerWrapper group relative rounded-xl overflow-hidden shadow-2xl border border-white/10"
              >
                <video
                  ref="mainVideoRef"
                  class="w-full h-full object-cover bg-black"
                  playsinline
                  preload="metadata"
                  :src="selectedVideoSource"
                  @loadedmetadata="handleMainVideoLoadedMetadata"
                  @timeupdate="updatePlayerControls"
                  @play="updatePlayerControls"
                  @pause="updatePlayerControls"
                  @volumechange="updatePlayerControls"
                  @ratechange="updatePlayerControls"
                ></video>
                <div
                  class="absolute inset-0 flex items-center justify-center opacity-0 pointer-events-none transition-opacity duration-200 group-hover:opacity-100 group-hover:pointer-events-auto"
                >
                  <button
                    class="w-20 h-20 bg-black/45 rounded-full flex items-center justify-center border border-white/20 hover:scale-105 transition-transform shadow-2xl"
                    @click="togglePlayerPlayback"
                  >
                    <span
                      class="material-symbols-outlined text-white text-[48px]"
                      style="font-variation-settings: 'FILL' 1"
                      >{{ playerPaused ? 'play_arrow' : 'pause' }}</span
                    >
                  </button>
                </div>
                <div
                  class="finished-playback-controls absolute inset-x-0 bottom-0 px-4 pt-10 pb-4"
                  :class="{ hidden: !showFinishedControls }"
                >
                  <div
                    class="finished-progress-track mb-3 cursor-pointer"
                    @click="seekPlayerTo"
                  >
                    <div
                      class="finished-progress-fill"
                      :style="{ width: `${playerProgress}%` }"
                    ></div>
                  </div>
                  <div class="flex items-center gap-3 text-white">
                    <button
                      class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
                      @click="seekPlayerBy(-10)"
                    >
                      <span class="material-symbols-outlined text-[18px]"
                        >replay_10</span
                      >
                    </button>
                    <button
                      class="w-9 h-9 rounded-full bg-electric-blue hover:brightness-110 flex items-center justify-center shadow-lg shadow-electric-blue/20 transition-all"
                      @click="togglePlayerPlayback"
                    >
                      <span
                        class="material-symbols-outlined text-[22px]"
                        style="font-variation-settings: 'FILL' 1"
                        >{{ playerPaused ? 'play_arrow' : 'pause' }}</span
                      >
                    </button>
                    <button
                      class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
                      @click="seekPlayerBy(10)"
                    >
                      <span class="material-symbols-outlined text-[18px]"
                        >forward_10</span
                      >
                    </button>
                    <div class="text-[11px] font-code-data text-white/80">
                      {{ playerTimeLabel }}
                    </div>
                    <div class="flex-1"></div>
                    <button
                      class="px-2.5 h-8 rounded bg-white/10 hover:bg-white/20 text-[11px] font-bold transition-colors"
                      @click="cyclePlayerSpeed"
                    >
                      {{ playerSpeed.toFixed(1) }}x
                    </button>
                    <button
                      class="w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
                      @click="togglePlayerMute"
                    >
                      <span class="material-symbols-outlined text-[18px]">{{
                        playerMuted ? 'volume_off' : 'volume_up'
                      }}</span>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            class="timeline-dock relative shrink-0"
            :class="{ 'timeline-collapsed': timelineCollapsed }"
          >
            <button
              v-if="timelineToggleVisible"
              class="absolute left-1/2 -translate-x-1/2 z-[70] h-7 w-12 rounded-t-full bg-surface-container-high border border-outline-variant text-white shadow-lg flex items-center justify-center transition-all duration-200"
              :class="timelineCollapsed ? 'top-[-28px]' : 'top-0'"
              @click="toggleTimelineContainer"
            >
              <span class="material-symbols-outlined text-[20px]">{{
                timelineCollapsed ? 'keyboard_arrow_up' : 'keyboard_arrow_down'
              }}</span>
              <span class="sr-only">展开时间线</span>
            </button>
            <div
              class="timeline-panel bg-surface-container-lowest border-t border-outline-variant flex flex-col"
            >
              <div
                class="track-area custom-scrollbar overflow-x-auto"
                style="background: #030d25; padding: 26px 12px 60px"
              >
                <div
                  class="track w-full"
                  style="
                    background: #07122a;
                    border: 1px solid rgba(74, 142, 255, 0.15);
                    border-radius: 4px;
                    margin-bottom: 8px;
                  "
                >
                  <div
                    class="track-title flex items-center gap-2"
                    style="
                      font-size: 10px;
                      font-weight: 700;
                      color: rgba(217, 226, 255, 0.5);
                      border-bottom: 1px solid rgba(74, 142, 255, 0.15);
                      background: rgba(16, 27, 51, 0.8);
                    "
                  >
                    {{ selectedStyleName }}
                  </div>
                  <div class="timeline-overview">
                    <div
                      ref="timelineRulerRef"
                      class="timeline-ruler"
                      aria-label="时间刻度"
                    >
                      <span
                        v-for="tick in timelineRulerMinorTicks"
                        :key="`minor-${tick.value}`"
                        class="timeline-ruler-tick is-minor"
                        :style="{ left: tick.left }"
                      ></span>
                      <span
                        v-for="tick in timelineRulerMajorTicks"
                        :key="`major-${tick.value}`"
                        class="timeline-ruler-tick is-major"
                        :style="{ left: tick.left }"
                      ></span>
                      <span
                        v-for="tick in timelineRulerMajorTicks"
                        :key="`label-${tick.value}`"
                        class="timeline-ruler-label"
                        :class="{
                          'is-start': tick.value === 0,
                          'is-end':
                            Math.abs(
                              tick.value - timelineRulerScale.totalDuration,
                            ) < 0.0001,
                        }"
                        :style="{ left: tick.left }"
                      >
                        {{ formatTimelineRulerTick(tick.value) }}
                      </span>
                    </div>
                    <button
                      class="timeline-playhead"
                      :class="{ 'is-dragging': timelinePlayheadDragging }"
                      :style="{ left: timelinePlayheadPercent }"
                      type="button"
                      aria-label="时间轴播放指针"
                      @pointerdown="startTimelinePlayheadDrag"
                    >
                      <span class="timeline-playhead-handle"></span>
                      <span class="timeline-playhead-line"></span>
                    </button>
                    <div class="clips-row relative h-16">
                      <div ref="timelineTrackRef" class="duration-track">
                        <div
                          class="duration-selection bg-electric-blue/20 border-2 border-electric-blue rounded-md flex items-center px-3 shadow-[0_0_15px_rgba(74,142,255,0.25)]"
                          :class="{ 'is-dragging': timelineDragging }"
                          :style="{
                            ...timelineSelectionStyle,
                            boxShadow:
                              timelinePulse && !timelineDragging
                                ? '0 0 20px rgba(74,142,255,0.45)'
                                : '0 0 15px rgba(74,142,255,0.25)',
                          }"
                          @pointerdown="startTimelineDrag"
                        >
                          <div
                            class="flex flex-col justify-between h-full py-1 px-2 w-full min-w-0"
                          >
                            <div class="flex items-center gap-1.5 min-w-0">
                              <span
                                class="text-[12px] font-bold text-white tracking-wide truncate"
                                >{{ selectedClipTitle }}</span
                              >
                            </div>
                            <div class="duration-meta-row">
                              <span
                                class="duration-meta-duration text-[10px] font-bold text-primary"
                                >时长：{{ timelineSelectedDurationLabel }}</span
                              >
                              <span
                                class="duration-meta-range text-[10px] font-medium text-white/90 font-code-data tracking-tighter"
                                >{{ timelineRangeLabel }}</span
                              >
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                <div
                  class="w-full flex items-center gap-3 px-4 py-3 bg-surface-container-low border border-outline-variant rounded"
                >
                  <label
                    class="text-[12px] font-bold text-on-surface-variant shrink-0 whitespace-nowrap"
                    for="subtitle-edit-input"
                    >标题编辑</label
                  >
                  <input
                    id="subtitle-edit-input"
                    v-model="subtitleText"
                    class="w-[360px] max-w-[42vw] h-9 bg-surface-container-lowest/50 border border-outline-variant/30 rounded px-3 text-[12px] text-on-surface placeholder:text-on-surface-variant/50 focus:border-electric-blue outline-none transition-colors"
                    placeholder="输入标题内容"
                    type="text"
                  />
                  <button
                    class="h-9 px-4 bg-electric-blue text-white rounded text-[12px] font-bold shadow-lg shadow-electric-blue/20 hover:brightness-110 active:scale-95 transition-all shrink-0"
                    :disabled="subtitleApplying"
                    @click="applySubtitleChange"
                  >
                    {{ subtitleApplying ? '应用中...' : '应用更改' }}
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div
            class="absolute inset-0 z-[200] flex items-center justify-center"
            :class="{ hidden: !previewModalVisible }"
          >
            <div class="relative w-full max-w-4xl mx-4">
              <div
                class="absolute -top-12 right-0 z-20 flex items-center gap-3"
              >
                <button
                  class="p-2 bg-black/40 backdrop-blur-md rounded-full transition-colors group"
                  :class="
                    previewFavorited
                      ? 'text-yellow-400 hover:text-yellow-300'
                      : 'text-white/80 hover:text-white'
                  "
                  type="button"
                  :disabled="
                    isTemplateFavoriteUpdating({ id: activeTemplateId })
                  "
                  @click.stop="togglePreviewFavorite"
                >
                  <span
                    class="material-symbols-outlined text-[24px] group-hover:scale-110"
                    :class="{
                      'start-editing-spinner': isTemplateFavoriteUpdating({
                        id: activeTemplateId,
                      }),
                    }"
                    :style="
                      previewFavorited
                        ? { fontVariationSettings: `'FILL' 1` }
                        : undefined
                    "
                    >{{
                      isTemplateFavoriteUpdating({ id: activeTemplateId })
                        ? 'progress_activity'
                        : 'star'
                    }}</span
                  >
                </button>
                <button
                  class="p-2 bg-black/40 backdrop-blur-md rounded-full text-white/80 hover:text-white transition-colors group"
                  type="button"
                  @click="hidePreviewModal"
                >
                  <span
                    class="material-symbols-outlined text-[24px] group-hover:scale-110"
                    >close</span
                  >
                </button>
              </div>
              <div
                class="relative w-full bg-surface-container rounded-2xl overflow-hidden shadow-[0_30px_90px_rgba(0,0,0,0.8)] border border-electric-blue/20 modal-pop-in"
              >
                <div
                  class="aspect-video bg-black flex items-center justify-center relative group cursor-pointer"
                >
                  <video
                    ref="modalVideoRef"
                    class="w-full h-full object-cover"
                    playsinline
                    preload="metadata"
                    :src="activeTemplateDemoSource"
                    @loadedmetadata="handleModalVideoLoadedMetadata"
                    @timeupdate="updateModalPreviewControls"
                    @play="updateModalPreviewControls"
                    @pause="updateModalPreviewControls"
                    @ended="updateModalPreviewControls"
                  ></video>
                  <div
                    class="absolute inset-0 flex items-center justify-center opacity-0 pointer-events-none transition-opacity duration-200 group-hover:opacity-100 group-hover:pointer-events-auto"
                  >
                    <button
                      class="w-20 h-20 bg-black/45 rounded-full flex items-center justify-center border border-white/20 hover:scale-105 transition-transform shadow-2xl"
                      @click="toggleModalPreviewPlayback"
                    >
                      <span
                        class="material-symbols-outlined text-white text-[48px]"
                        style="font-variation-settings: 'FILL' 1"
                        >{{ modalPaused ? 'play_arrow' : 'pause' }}</span
                      >
                    </button>
                  </div>
                  <div
                    class="modal-preview-progress absolute inset-x-4 bottom-4 cursor-pointer"
                    :class="{ 'is-dragging': modalPreviewProgressDragging }"
                    @pointerdown="startModalPreviewProgressDrag"
                  >
                    <div
                      class="finished-progress-fill"
                      :style="{ width: `${modalProgress}%` }"
                    ></div>
                    <span
                      class="modal-preview-progress-thumb"
                      :style="{ left: `${modalProgress}%` }"
                    ></span>
                  </div>
                </div>
                <div
                  class="p-6 flex items-center justify-between bg-surface-container-lowest/80 border-t border-white/5"
                >
                  <div class="min-w-0 flex-1">
                    <h3 class="text-lg font-black text-on-surface">
                      {{ previewTitle }}
                    </h3>
                  </div>
                  <div
                    class="flex items-center justify-center gap-16 px-8 py-2 rounded-full bg-white/5 border border-white/10 min-w-[420px]"
                  >
                    <button
                      class="w-16 h-9 rounded-full bg-white/5 border border-white/10 text-[12px] font-bold text-white/80 hover:bg-white/10 hover:text-white transition-colors"
                      type="button"
                      @click="cycleModalPlaybackRate"
                    >
                      {{ modalPlaybackRate }}x
                    </button>
                    <button
                      class="w-11 h-11 rounded-full bg-electric-blue text-white hover:brightness-110 active:scale-95 transition-all shadow-lg shadow-electric-blue/20 flex items-center justify-center"
                      type="button"
                      @click="toggleModalPreviewPlayback"
                    >
                      <span
                        class="material-symbols-outlined text-[28px]"
                        style="font-variation-settings: 'FILL' 1"
                        >{{ modalPaused ? 'play_arrow' : 'pause' }}</span
                      >
                    </button>
                    <button
                      class="w-16 h-9 rounded-full bg-white/5 border border-white/10 text-white/80 hover:bg-white/10 hover:text-white transition-colors flex items-center justify-center"
                      type="button"
                      @click="seekModalPreviewBy(5)"
                    >
                      <span class="material-symbols-outlined text-[22px]"
                        >fast_forward</span
                      >
                    </button>
                  </div>
                  <div class="flex items-center gap-4 flex-1 justify-end">
                    <button
                      class="px-10 py-3 bg-electric-blue text-white rounded-lg font-bold text-[16px] shadow-2xl shadow-electric-blue/30 hover:brightness-110 active:scale-95 transition-all uppercase disabled:opacity-70 disabled:cursor-not-allowed disabled:active:scale-100 inline-flex items-center gap-2"
                      :disabled="startEditingLoading"
                      @click="confirmSelection"
                    >
                      <span
                        v-if="startEditingLoading"
                        class="material-symbols-outlined text-[20px] start-editing-spinner"
                        >progress_activity</span
                      >
                      {{ startEditingLoading ? '正在加载' : '开始编辑' }}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[440] flex items-center justify-center transition-all duration-300"
            :class="{ hidden: !helpCenterVisible }"
          >
            <div
              class="absolute inset-0 bg-black/60 backdrop-blur-sm"
              @click="hideHelpCenter"
            ></div>
            <div
              class="relative w-[90vw] h-[85vh] bg-surface-container rounded-3xl border border-white/10 shadow-[0_32px_64px_-12px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col modal-pop-in"
            >
              <div
                class="h-16 shrink-0 bg-surface-container-highest border-b border-white/10 px-8 flex items-center justify-between"
              >
                <div class="flex items-center gap-3">
                  <div
                    class="w-8 h-8 bg-electric-blue rounded-lg flex items-center justify-center"
                  >
                    <span
                      class="material-symbols-outlined text-white text-[20px]"
                      >menu_book</span
                    >
                  </div>
                  <h3 class="text-lg font-black text-white">产品帮助中心</h3>
                </div>
                <button
                  class="p-2 hover:bg-white/10 rounded-full text-on-surface-variant transition-colors"
                  type="button"
                  @click="hideHelpCenter"
                >
                  <span class="material-symbols-outlined">close</span>
                </button>
              </div>
              <div class="flex-1 flex overflow-hidden">
                <aside
                  class="w-64 shrink-0 bg-black/20 border-r border-white/5 p-4 flex flex-col gap-1"
                >
                  <button
                    class="help-nav-item flex items-center gap-3 px-4 py-3 rounded-xl transition-all text-left"
                    :class="
                      activeHelpTab === 'guide'
                        ? 'bg-electric-blue/10 text-electric-blue font-bold'
                        : 'text-on-surface-variant hover:bg-white/5'
                    "
                    type="button"
                    @click="switchHelpTab('guide')"
                  >
                    <span class="material-symbols-outlined text-[20px]"
                      >explore</span
                    >
                    <span>使用指南</span>
                  </button>
                  <button
                    class="help-nav-item flex items-center gap-3 px-4 py-3 rounded-xl transition-all text-left"
                    :class="
                      activeHelpTab === 'faq'
                        ? 'bg-electric-blue/10 text-electric-blue font-bold'
                        : 'text-on-surface-variant hover:bg-white/5'
                    "
                    type="button"
                    @click="switchHelpTab('faq')"
                  >
                    <span class="material-symbols-outlined text-[20px]"
                      >quiz</span
                    >
                    <span>常见问题</span>
                  </button>
                  <button
                    class="help-nav-item flex items-center gap-3 px-4 py-3 rounded-xl transition-all text-left"
                    :class="
                      activeHelpTab === 'changelog'
                        ? 'bg-electric-blue/10 text-electric-blue font-bold'
                        : 'text-on-surface-variant hover:bg-white/5'
                    "
                    type="button"
                    @click="switchHelpTab('changelog')"
                  >
                    <span class="material-symbols-outlined text-[20px]"
                      >history</span
                    >
                    <span>更新日志</span>
                  </button>
                </aside>
                <main
                  class="flex-1 flex flex-col bg-surface-container-lowest p-8 overflow-hidden relative"
                >
                  <div
                    class="flex-1 bg-surface-container rounded-2xl border border-white/5 shadow-inner overflow-y-auto custom-scrollbar p-12"
                  >
                    <div
                      v-if="activeHelpTab === 'guide'"
                      class="max-w-2xl mx-auto space-y-8"
                    >
                      <div class="space-y-4">
                        <h1 class="text-3xl font-black text-white">
                          核心使用指南 V1.0
                        </h1>
                        <p class="text-on-surface-variant leading-relaxed">
                          欢迎使用 AICut
                          专业版剪辑系统。这里可以快速了解模板选择、素材导入、开始编辑和导出成片的核心流程。
                        </p>
                      </div>
                      <div class="grid grid-cols-2 gap-6">
                        <div
                          class="p-6 bg-white/5 rounded-2xl border border-white/10"
                        >
                          <h4 class="text-electric-blue font-bold mb-2">
                            智能模板匹配
                          </h4>
                          <p class="text-[13px] text-on-surface-variant/80">
                            选择模板后，系统会按模板素材位展示所需视频数量和时长范围。
                          </p>
                        </div>
                        <div
                          class="p-6 bg-white/5 rounded-2xl border border-white/10"
                        >
                          <h4 class="text-electric-blue font-bold mb-2">
                            一键批量导入
                          </h4>
                          <p class="text-[13px] text-on-surface-variant/80">
                            可以按分类批量替换素材，也可以单独替换某一个素材位。
                          </p>
                        </div>
                      </div>
                    </div>
                    <div
                      v-else-if="activeHelpTab === 'faq'"
                      class="max-w-2xl mx-auto space-y-6"
                    >
                      <h1 class="text-3xl font-black text-white">常见问题</h1>
                      <div class="space-y-4">
                        <div
                          class="p-5 bg-white/5 rounded-2xl border border-white/10"
                        >
                          <h4 class="text-white font-bold mb-2">
                            导入素材后为什么不能导出？
                          </h4>
                          <p class="text-[13px] text-on-surface-variant/80">
                            需要先点击开始编辑，生成工程文件后导出按钮才会可用。
                          </p>
                        </div>
                        <div
                          class="p-5 bg-white/5 rounded-2xl border border-white/10"
                        >
                          <h4 class="text-white font-bold mb-2">
                            时间滑块有什么作用？
                          </h4>
                          <p class="text-[13px] text-on-surface-variant/80">
                            时间滑块用于选择素材在原视频中的起始片段，工程文件会同步记录偏移时间。
                          </p>
                        </div>
                      </div>
                    </div>
                    <div v-else class="max-w-2xl mx-auto space-y-6">
                      <h1 class="text-3xl font-black text-white">更新日志</h1>
                      <div
                        class="p-5 bg-white/5 rounded-2xl border border-white/10"
                      >
                        <h4 class="text-electric-blue font-bold mb-2">
                          当前版本
                        </h4>
                        <p class="text-[13px] text-on-surface-variant/80">
                          优化模板素材导入、时间轴偏移、工程创建和导出流程。
                        </p>
                      </div>
                    </div>
                  </div>
                  <div class="absolute bottom-12 right-12">
                    <button
                      class="flex items-center gap-2 px-6 py-3 bg-electric-blue text-white rounded-full font-black shadow-2xl shadow-electric-blue/40 hover:scale-105 active:scale-95 transition-all"
                      type="button"
                      @click="downloadHelpGuide"
                    >
                      <span class="material-symbols-outlined">download</span>
                      <span>下载离线指南</span>
                    </button>
                  </div>
                </main>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[430] flex items-center justify-center transition-opacity duration-300"
            :class="{ hidden: !profileModalVisible }"
          >
            <div
              class="absolute inset-0 bg-black/60 backdrop-blur-sm"
              @click="hideProfileModal"
            ></div>
            <div
              class="relative w-full max-w-md bg-surface-container-highest rounded-2xl border border-white/10 shadow-2xl modal-pop-in p-8 text-center"
            >
              <button
                class="absolute top-4 right-4 text-on-surface-variant hover:text-white transition-colors"
                type="button"
                @click="hideProfileModal"
              >
                <span class="material-symbols-outlined">close</span>
              </button>
              <div
                class="w-16 h-16 bg-electric-blue/10 rounded-full flex items-center justify-center mx-auto mb-4"
              >
                <span
                  class="material-symbols-outlined text-electric-blue text-3xl"
                  >person</span
                >
              </div>
              <h3 class="text-xl font-black text-white mb-6">个人信息</h3>
              <div
                class="space-y-3 text-left bg-black/20 p-5 rounded-xl border border-white/5"
              >
                <div class="flex justify-between gap-4">
                  <span class="text-on-surface-variant/60 text-xs shrink-0"
                    >账号</span
                  >
                  <span class="text-white text-xs font-bold truncate">{{
                    accountDisplayName
                  }}</span>
                </div>
                <div class="flex justify-between gap-4">
                  <span class="text-on-surface-variant/60 text-xs shrink-0"
                    >昵称</span
                  >
                  <span class="text-white text-xs font-bold truncate">{{
                    accountVersionName
                  }}</span>
                </div>
                <div class="flex justify-between gap-4">
                  <span class="text-on-surface-variant/60 text-xs shrink-0"
                    >所属租户</span
                  >
                  <span class="text-electric-blue text-xs font-bold truncate">{{
                    accountTenantName
                  }}</span>
                </div>
                <div
                  class="pt-3 border-t border-white/10 flex justify-between items-baseline gap-4"
                >
                  <span class="text-on-surface-variant/60 text-xs shrink-0"
                    >剩余积分</span
                  >
                  <span class="text-2xl font-black text-electric-blue"
                    >{{ accountBalance }}
                    <span class="text-[10px]">pts</span></span
                  >
                </div>
              </div>
              <button
                class="w-full mt-6 py-3 bg-white/5 text-on-surface-variant font-bold rounded-xl hover:text-white hover:bg-white/10 transition-all"
                type="button"
                @click="hideProfileModal"
              >
                关闭
              </button>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[430] flex items-center justify-center transition-opacity duration-300"
            :class="{ hidden: !passwordModalVisible }"
          >
            <div
              class="absolute inset-0 bg-black/60 backdrop-blur-sm"
              @click="hidePasswordModal"
            ></div>
            <div
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl border border-white/10 shadow-2xl modal-pop-in p-6"
            >
              <h3
                class="text-lg font-black text-white mb-6 flex items-center gap-2"
              >
                <span class="material-symbols-outlined text-electric-blue"
                  >lock_reset</span
                >
                修改密码
              </h3>
              <div class="space-y-4">
                <input
                  v-model="passwordForm.oldPassword"
                  class="w-full bg-surface-container-low border border-white/10 rounded-xl px-4 py-3 text-sm text-white focus:border-electric-blue/50 outline-none"
                  :disabled="passwordSubmitting"
                  placeholder="旧密码"
                  type="password"
                  @keydown.enter.prevent="submitPasswordReset"
                />
                <input
                  v-model="passwordForm.newPassword"
                  class="w-full bg-surface-container-low border border-white/10 rounded-xl px-4 py-3 text-sm text-white focus:border-electric-blue/50 outline-none"
                  :disabled="passwordSubmitting"
                  placeholder="新密码"
                  type="password"
                  @keydown.enter.prevent="submitPasswordReset"
                />
                <input
                  v-model="passwordForm.confirmPassword"
                  class="w-full bg-surface-container-low border border-white/10 rounded-xl px-4 py-3 text-sm text-white focus:border-electric-blue/50 outline-none"
                  :disabled="passwordSubmitting"
                  placeholder="确认新密码"
                  type="password"
                  @keydown.enter.prevent="submitPasswordReset"
                />
                <button
                  class="w-full py-3 bg-electric-blue text-white font-black rounded-xl hover:brightness-110 transition-all disabled:opacity-60 disabled:cursor-not-allowed"
                  type="button"
                  :disabled="passwordSubmitting"
                  @click="submitPasswordReset"
                >
                  {{ passwordSubmitting ? '正在保存...' : '保存修改' }}
                </button>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[450] flex items-center justify-center transition-opacity duration-300"
            :class="{ hidden: !logoutConfirmVisible }"
          >
            <div
              class="absolute inset-0 bg-black/60 backdrop-blur-sm"
              @click="hideLogoutConfirm"
            ></div>
            <div
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-8 border border-white/10 shadow-2xl modal-pop-in text-center"
            >
              <div
                class="w-16 h-16 bg-white/5 rounded-full flex items-center justify-center mx-auto mb-4 border border-white/10"
              >
                <span class="material-symbols-outlined text-[#ec4034] text-3xl"
                  >logout</span
                >
              </div>
              <h3 class="text-xl font-black text-white mb-2">退出登录</h3>
              <p class="text-on-surface-variant text-sm mb-6">
                确认退出当前账号吗？
              </p>
              <div class="flex flex-col gap-3">
                <button
                  class="w-full py-3 bg-[#ec4034] text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95 disabled:opacity-60 disabled:cursor-not-allowed disabled:active:scale-100"
                  type="button"
                  :disabled="logoutSubmitting"
                  @click="confirmLogout"
                >
                  {{ logoutSubmitting ? '正在退出...' : '确认退出' }}
                </button>
                <button
                  class="w-full py-3 bg-transparent text-on-surface-variant font-bold rounded-xl hover:text-white transition-colors disabled:opacity-60 disabled:cursor-not-allowed"
                  type="button"
                  :disabled="logoutSubmitting"
                  @click="hideLogoutConfirm"
                >
                  取消
                </button>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[410] flex items-center justify-center"
            :class="{ hidden: !importOverwriteConfirmVisible }"
          >
            <div class="absolute inset-0 bg-black/50"></div>
            <div
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-7 border border-white/10 shadow-2xl modal-pop-in"
            >
              <div class="flex flex-col items-center text-center gap-5">
                <div
                  class="w-14 h-14 rounded-full bg-electric-blue/10 border border-electric-blue/20 flex items-center justify-center"
                >
                  <span
                    class="material-symbols-outlined text-electric-blue text-3xl"
                    >sync_problem</span
                  >
                </div>
                <div>
                  <h3 class="text-lg font-black text-white mb-2">
                    确认一键导入
                  </h3>
                  <p class="text-[13px] leading-6 text-on-surface-variant">
                    当前类下已经导入过视频，一键导入会替换全部视频内容，是否继续？
                  </p>
                </div>
                <div class="grid grid-cols-2 gap-3 w-full">
                  <button
                    class="h-11 rounded-xl bg-white/5 text-on-surface-variant text-[13px] font-bold hover:bg-white/10 hover:text-white transition-all active:scale-95"
                    type="button"
                    @click="closeImportOverwriteConfirm"
                  >
                    取消
                  </button>
                  <button
                    class="h-11 rounded-xl bg-electric-blue text-white text-[13px] font-bold hover:brightness-110 transition-all active:scale-95 shadow-lg shadow-electric-blue/20"
                    type="button"
                    @click="confirmImportOverwrite"
                  >
                    确定
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[420] flex items-center justify-center"
            :class="{ hidden: !templateDownloadVisible }"
          >
            <div class="absolute inset-0 bg-black/60 backdrop-blur-2xl"></div>
            <div
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-8 border border-white/10 shadow-2xl modal-pop-in"
            >
              <div class="flex flex-col items-center text-center space-y-6">
                <div
                  class="w-16 h-16 bg-electric-blue/10 rounded-full flex items-center justify-center"
                >
                  <span
                    class="material-symbols-outlined text-electric-blue text-3xl"
                    >downloading</span
                  >
                </div>
                <div
                  class="circular-progress"
                  :style="{ '--progress': `${templateDownloadProgress}%` }"
                >
                  <div
                    class="absolute inset-0 flex items-center justify-center"
                  >
                    <span class="text-2xl font-black text-electric-blue"
                      >{{ Math.round(templateDownloadProgress) }}%</span
                    >
                  </div>
                </div>
                <div>
                  <h3 class="text-xl font-black text-white mb-2">
                    正在下载模板资源
                  </h3>
                  <p
                    class="text-on-surface-variant text-sm truncate max-w-[280px]"
                  >
                    {{ templateDownloadTitle }}
                  </p>
                  <p class="text-on-surface-variant/70 text-xs mt-3">
                    {{ templateDownloadStatus }}
                  </p>
                </div>
                <button
                  class="w-full py-3 bg-white/5 text-on-surface-variant font-bold rounded-xl hover:bg-white/10 hover:text-white transition-all disabled:opacity-60 disabled:cursor-not-allowed"
                  type="button"
                  :disabled="templateDownloadCanceling"
                  @click="cancelTemplateDownload"
                >
                  {{ templateDownloadCanceling ? '正在取消...' : '取消下载' }}
                </button>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[410] flex items-center justify-center"
            :class="{ hidden: !defaultTemplateExportConfirmVisible }"
          >
            <div class="absolute inset-0 bg-black/60 backdrop-blur-2xl"></div>
            <div
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-8 border border-white/10 shadow-2xl modal-pop-in"
            >
              <div class="flex flex-col items-center text-center space-y-6">
                <div
                  class="w-16 h-16 bg-electric-blue/10 rounded-full flex items-center justify-center"
                >
                  <span
                    class="material-symbols-outlined text-electric-blue text-3xl"
                    >warning</span
                  >
                </div>
                <div>
                  <h3 class="text-xl font-black text-white mb-2">确认导出</h3>
                  <p class="text-on-surface-variant text-sm">
                    当前工程存在未替换的默认模板视频，是否导出？
                  </p>
                </div>
                <div class="flex w-full gap-3">
                  <button
                    class="flex-1 py-3 bg-transparent text-on-surface-variant font-bold rounded-xl hover:text-white transition-colors"
                    type="button"
                    @click="cancelDefaultTemplateExport"
                  >
                    取消
                  </button>
                  <button
                    class="flex-1 py-3 bg-electric-blue text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95 shadow-lg shadow-electric-blue/20"
                    type="button"
                    @click="confirmDefaultTemplateExport"
                  >
                    确定
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div
            class="fixed inset-0 z-[400] flex items-center justify-center"
            :class="{ hidden: !exportModalVisible }"
          >
            <div class="absolute inset-0 bg-black/60 backdrop-blur-2xl"></div>
            <div
              v-if="exportState === 'confirm'"
              class="relative w-full max-w-sm bg-surface-container-highest rounded-2xl p-8 border border-white/10 shadow-2xl modal-pop-in"
            >
              <div class="flex flex-col items-center text-center space-y-6">
                <div
                  class="w-16 h-16 bg-electric-blue/10 rounded-full flex items-center justify-center"
                >
                  <span
                    class="material-symbols-outlined text-electric-blue text-3xl"
                    >payments</span
                  >
                </div>
                <div>
                  <h3 class="text-xl font-black text-white mb-2">
                    确认导出工程
                  </h3>
                  <p
                    v-if="!activeProjectExported"
                    class="text-on-surface-variant text-sm"
                  >
                    本次导出将扣除
                    <span class="text-electric-blue font-bold">
                      {{ activeTemplateExportCredit }} 积分
                    </span>
                  </p>
                  <p
                    class="mt-2 text-[11px] text-on-surface-variant/70 break-all"
                  >
                    {{ exportSelectedDir }}
                  </p>
                </div>
                <div class="flex flex-col w-full gap-3">
                  <button
                    class="w-full py-3 bg-electric-blue text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95 shadow-lg shadow-electric-blue/20"
                    @click="startExportProgress"
                  >
                    确认导出
                  </button>
                  <button
                    class="w-full py-3 bg-transparent text-on-surface-variant font-bold rounded-xl hover:text-white transition-colors"
                    @click="closeExportModal"
                  >
                    再想想
                  </button>
                </div>
              </div>
            </div>
            <div
              v-else
              class="relative w-full max-w-md bg-surface-container-highest rounded-2xl p-10 border border-white/10 shadow-2xl modal-pop-in"
            >
              <div class="flex flex-col items-center space-y-8">
                <div
                  class="circular-progress"
                  :style="{ '--progress': `${exportProgress}%` }"
                >
                  <div
                    class="absolute inset-0 flex items-center justify-center"
                  >
                    <span class="text-2xl font-black text-electric-blue"
                      >{{ exportProgress }}%</span
                    >
                  </div>
                </div>
                <div class="text-center">
                  <h3 class="text-lg font-bold text-white mb-1">
                    {{ exportStatus }}
                  </h3>
                  <p
                    v-if="exportOutputPath"
                    class="mt-2 max-w-[320px] break-all text-[11px] text-on-surface-variant/70"
                  >
                    {{ exportOutputPath }}
                  </p>
                </div>
                <div class="w-full space-y-4">
                  <div
                    class="h-1.5 w-full bg-white/5 rounded-full overflow-hidden"
                  >
                    <div
                      class="h-full bg-electric-blue transition-all duration-300 shadow-[0_0_12px_rgba(74,142,255,0.8)]"
                      :style="{ width: `${exportProgress}%` }"
                    ></div>
                  </div>
                  <button
                    v-if="exportProgress < 100"
                    class="w-full py-2.5 bg-white/5 text-on-surface-variant text-sm font-bold rounded-lg hover:bg-white/10 hover:text-white transition-all"
                    :disabled="exportRunning"
                    @click="closeExportModal"
                  >
                    {{ exportRunning ? '导出中...' : '关闭' }}
                  </button>
                  <template v-else>
                    <button
                      class="w-full py-3 bg-white/5 text-white font-bold rounded-xl hover:bg-white/10 transition-all active:scale-95 border border-white/10"
                      type="button"
                      @click="openExportDirectory"
                    >
                      打开导出目录
                    </button>
                    <button
                      class="w-full py-3 bg-electric-blue text-white font-bold rounded-xl hover:brightness-110 transition-all active:scale-95"
                      type="button"
                      @click="closeExportModal"
                    >
                      完成
                    </button>
                  </template>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>
    </main>

    <div
      class="absolute top-[120px] left-0 right-0 bottom-0 bg-background flex flex-col z-[210]"
      :class="{ hidden: !draftLibraryVisible }"
    >
      <div
        class="p-6 border-b border-white/5 flex items-center justify-between bg-surface-container/80 backdrop-blur-3xl"
      >
        <div class="flex items-center gap-4">
          <button
            class="flex items-center gap-2 text-on-surface-variant hover:text-electric-blue bg-surface-container-lowest px-3 py-1.5 rounded-lg border border-white/5 transition-all"
            @click="hideDraftLibrary"
          >
            <span class="material-symbols-outlined text-[20px]">arrow_back</span
            ><span class="text-[12px] font-bold">返回</span>
          </button>
          <h2 class="text-xl font-black text-white flex items-center gap-2">
            工程库
          </h2>
        </div>
        <div class="flex items-end gap-2">
          <span
            class="h-9 flex items-center text-[12px] font-bold text-on-surface-variant"
            >分类</span
          >
          <div class="relative">
            <select
              v-model="draftFilter"
              class="draft-filter-select appearance-none h-9 min-w-[112px] bg-surface-container-lowest border border-white/5 rounded-md pl-3 pr-9 py-0 text-[12px] font-bold text-on-surface outline-none hover:border-electric-blue/50 focus:border-electric-blue transition-colors shadow-inner"
            >
              <option value="all">全部</option>
              <option value="editing">编辑中</option>
              <option value="exported">已导出</option>
            </select>
            <span
              class="material-symbols-outlined pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[18px] text-electric-blue"
              >expand_more</span
            >
          </div>
          <button
            class="h-9 px-3 rounded-md border border-white/5 bg-surface-container-lowest text-[12px] font-bold text-on-surface-variant hover:border-electric-blue/50 hover:text-electric-blue transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            type="button"
            :disabled="draftDeleting"
            @click="toggleDraftBatchDeleteMode"
          >
            {{ draftBatchDeleteMode ? '确定' : '批量删除' }}
          </button>
          <button
            v-if="draftBatchDeleteMode"
            class="h-9 px-3 rounded-md border border-white/5 bg-surface-container-lowest text-[12px] font-bold text-on-surface-variant hover:border-white/20 hover:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            type="button"
            :disabled="draftDeleting"
            @click="resetDraftBatchDelete"
          >
            取消
          </button>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-6">
        <div
          v-if="visibleDraftProjects.length === 0"
          class="h-full min-h-[260px] flex items-center justify-center text-on-surface-variant text-sm"
        >
          暂无工程
        </div>
        <div
          class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 templateListWrapper"
        >
          <div
            v-for="project in visibleDraftProjects"
            :key="project.id"
            class="draft-project-card group relative aspect-[16/9] bg-surface-container-high rounded-xl overflow-hidden border border-white/5 hover:border-electric-blue/60 transition-all cursor-pointer shadow-lg hover:shadow-electric-blue/10"
            @click="
              draftBatchDeleteMode
                ? toggleDraftProjectSelection(project)
                : openDraftProject(project.id)
            "
          >
            <label
              v-if="draftBatchDeleteMode && getDraftProjectDeleteId(project)"
              class="absolute left-3 top-3 z-20 flex h-9 w-9 items-center justify-center cursor-pointer"
              @click.stop
            >
              <input
                class="sr-only"
                type="checkbox"
                :checked="isDraftProjectSelected(project)"
                @change="toggleDraftProjectSelection(project)"
              />
              <span
                class="material-symbols-outlined text-[26px]"
                :class="
                  isDraftProjectSelected(project)
                    ? 'text-electric-blue'
                    : 'text-white/50'
                "
                >{{
                  isDraftProjectSelected(project)
                    ? 'check_box'
                    : 'check_box_outline_blank'
                }}</span
              >
            </label>
            <img
              v-if="project.image"
              alt="draft preview"
              class="w-full h-full object-cover opacity-70 group-hover:scale-105 transition-transform duration-500 cursor-pointer"
              :src="project.image"
            />
            <div
              class="absolute inset-0 flex flex-col justify-end p-4 pointer-events-none"
            >
              <input
                v-if="!draftBatchDeleteMode && editingDraftId === project.id"
                :ref="(element) => setDraftTitleInputRef(project.id, element)"
                v-model="draftEditTitle"
                class="pointer-events-auto max-w-[180px] rounded bg-surface-container-lowest/80 border border-electric-blue/60 px-1.5 py-0.5 text-[12px] font-bold text-white outline-none disabled:opacity-60"
                :disabled="Boolean(draftRenamingId)"
                aria-label="编辑工程名称"
                @click.stop
                @keydown.enter.prevent="saveDraftTitle(project)"
                @keydown.esc.prevent="resetDraftTitleEdit"
                @blur="resetDraftTitleEdit"
              />
              <button
                v-else-if="!draftBatchDeleteMode"
                class="draft-title-btn pointer-events-auto text-left text-[14px] font-black text-white truncate hover:text-electric-blue transition-colors"
                type="button"
                title="点击编辑工程名称"
                @click.stop="startDraftTitleEdit(project)"
              >
                {{ project.title }}
              </button>
              <div
                v-else
                class="text-left text-[14px] font-black text-white truncate"
              >
                {{ project.title }}
              </div>
              <div class="text-[10px] text-white/60">
                {{ statusMeta(project.status).detail }} ·
                {{ project.duration }} · {{ project.time }}
              </div>
            </div>
            <div
              class="absolute top-3 right-3 text-[11px] font-black px-3 py-1 rounded-md shadow-lg"
              :class="statusMeta(project.status).className"
            >
              {{ statusMeta(project.status).label }}
            </div>
            <button
              v-if="!draftBatchDeleteMode && getDraftProjectDeleteId(project)"
              class="absolute bottom-3 right-3 z-20 flex h-6 w-6 items-center justify-center rounded bg-[#ec4034] text-white"
              type="button"
              title="删除工程"
              @click.stop="requestSingleDraftDelete(project)"
            >
              <span class="material-symbols-outlined text-[15px]">delete</span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <div
      class="fixed inset-0 z-[260] flex items-center justify-center bg-black/60 backdrop-blur-sm"
      :class="{ hidden: !draftDeleteConfirmVisible }"
    >
      <div
        class="w-[360px] max-w-[calc(100vw-32px)] rounded-2xl border border-white/10 bg-surface-container-high p-6 shadow-[0_24px_80px_rgba(0,0,0,0.45)]"
      >
        <h3 class="text-lg font-black text-white mb-3">确认删除</h3>
        <p class="text-[14px] leading-6 text-on-surface-variant">
          删除内容无法找回 是否确定删除？
        </p>
        <div class="mt-6 grid grid-cols-2 gap-3">
          <button
            class="h-10 rounded-lg border border-white/10 bg-white/5 text-[13px] font-bold text-on-surface-variant hover:bg-white/10 hover:text-white transition-colors"
            type="button"
            :disabled="draftDeleting"
            @click="cancelDraftDeleteConfirm"
          >
            取消
          </button>
          <button
            class="h-10 rounded-lg bg-[#ec4034] text-[13px] font-bold text-white hover:brightness-110 transition-all disabled:opacity-60 disabled:cursor-not-allowed"
            type="button"
            :disabled="draftDeleting"
            @click="confirmDraftBatchDelete"
          >
            {{ draftDeleting ? '删除中...' : '确定' }}
          </button>
        </div>
      </div>
    </div>

    <div
      class="absolute top-[120px] left-0 right-0 bottom-0 bg-background flex flex-col z-[220] overlay-view-enter"
      :class="{ hidden: !finishedLibraryVisible }"
    >
      <div
        class="p-6 border-b border-white/5 bg-surface-container/80 backdrop-blur-3xl space-y-4"
      >
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-4">
            <button
              class="flex items-center gap-2 text-on-surface-variant hover:text-electric-blue bg-surface-container-lowest px-3 py-1.5 rounded-lg border border-white/5 transition-all"
              type="button"
              @click="hideFinishedLibrary"
            >
              <span class="material-symbols-outlined text-[20px]"
                >arrow_back</span
              >
              <span class="text-[12px] font-bold">返回</span>
            </button>
            <h2 class="text-xl font-black flex items-center gap-2 text-white">
              <span>我的收藏</span>
            </h2>
          </div>
        </div>
        <!-- <div class="flex items-center gap-3">
          <span
            class="text-[11px] text-on-surface-variant/60 font-bold uppercase tracking-wider"
            >全部分类:</span
          >
          <div class="flex items-center gap-2 overflow-x-auto no-scrollbar">
            <button
              v-for="filter in favoriteCategoryFilters"
              :key="filter.key"
              class="px-4 py-1.5 rounded-full text-[11px] border transition-all whitespace-nowrap"
              :class="
                activeFavoriteFilter === filter.key
                  ? 'bg-electric-blue text-white font-black border-white shadow-lg shadow-electric-blue/20'
                  : 'bg-white/5 text-on-surface-variant font-bold border-white/5 hover:border-electric-blue/40'
              "
              type="button"
              @click="filterFavorites(filter.key)"
            >
              {{ filter.label }} ({{ filter.count }})
            </button>
          </div>
        </div> -->
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-8">
        <div
          v-if="favoritesLoading"
          class="h-full flex items-center justify-center text-on-surface-variant/70 text-[13px]"
        >
          收藏模板加载中...
        </div>
        <div
          v-else-if="favoriteLibraryItems.length"
          class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 xl:grid-cols-5 gap-6"
        >
          <button
            v-for="template in favoriteLibraryItems"
            :key="getTemplateFavoriteKey(template)"
            class="group relative aspect-[16/9] bg-surface-container-high rounded-xl overflow-hidden border border-white/5 hover:border-electric-blue/60 transition-all cursor-pointer shadow-lg text-left"
            type="button"
            @click="openFavoriteTemplate(template)"
          >
            <img
              v-if="template.image"
              class="w-full h-full object-cover opacity-70 group-hover:scale-105 transition-transform duration-500"
              :src="template.image"
              alt="favorite template preview"
            />
            <div
              class="absolute inset-0 bg-gradient-to-t from-black/90 via-transparent flex flex-col justify-end p-4"
            >
              <div class="text-[13px] font-black text-white truncate">
                {{ template.title }}
              </div>
              <div class="text-[10px] text-white/60 truncate">
                {{ template.subtitle }}
              </div>
            </div>
          </button>
        </div>
        <div
          v-else
          class="h-full min-h-[320px] flex flex-col items-center justify-center text-center"
        >
          <div
            class="w-16 h-16 rounded-2xl bg-white/5 border border-white/10 flex items-center justify-center mb-4"
          >
            <span class="material-symbols-outlined text-electric-blue text-3xl"
              >stars</span
            >
          </div>
          <h3 class="text-lg font-black text-white">暂无收藏模板</h3>
          <p class="text-[13px] text-on-surface-variant mt-2">
            在模板预览中点亮星标后，会显示在这里。
          </p>
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
    'FILL' 0,
    'wght' 400,
    'GRAD' 0,
    'opsz' 24;
  vertical-align: middle;
}

.templateListWrapper .draft-project-card {
  box-sizing: border-box;
  border-color: rgba(255, 255, 255, 0.12);
}

.templateListWrapper .draft-project-card:hover {
  border-color: #4a8eff !important;
  box-shadow:
    0 0 0 1px rgba(74, 142, 255, 0.35),
    0 12px 30px rgba(74, 142, 255, 0.1);
}

.draft-filter-select {
  line-height: 34px;
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

.duration-meta-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  width: 100%;
}

.duration-meta-row > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.duration-meta-duration {
  flex: 1 1 45%;
}

.duration-meta-range {
  flex: 1 1 55%;
  text-align: right;
}

.timeline-overview {
  position: relative;
  margin: 0 12px;
}

.timeline-overview .timeline-ruler {
  margin: 0;
}

.timeline-overview .clips-row {
  padding: 8px 0;
}

.timeline-playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 90;
  width: 18px;
  min-width: 18px;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: ew-resize;
  touch-action: none;
  transform: translateX(-50%);
}

.timeline-playhead-line {
  position: absolute;
  top: 8px;
  bottom: 0;
  left: 50%;
  width: 2px;
  transform: translateX(-50%);
  background: #4a8eff;
  box-shadow: 0 0 10px rgba(74, 142, 255, 0.55);
  pointer-events: none;
}

.timeline-playhead-handle {
  position: absolute;
  top: 0;
  left: 50%;
  width: 13px;
  height: 13px;
  transform: translateX(-50%);
  border-radius: 0 0 4px 4px;
  background: #4a8eff;
  box-shadow: 0 0 12px rgba(74, 142, 255, 0.5);
  pointer-events: none;
}

.timeline-playhead.is-dragging .timeline-playhead-line,
.timeline-playhead.is-dragging .timeline-playhead-handle {
  background: #4a8eff;
  box-shadow: 0 0 14px rgba(74, 142, 255, 0.72);
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

.timeline-ruler-tick {
  position: absolute;
  top: 0;
  width: 1px;
  background: rgba(217, 226, 255, 0.3);
  pointer-events: none;
}

.timeline-ruler-tick.is-minor {
  height: 8px;
}

.timeline-ruler-tick.is-major {
  height: 15px;
  background: rgba(217, 226, 255, 0.62);
}

.timeline-ruler-label {
  position: absolute;
  top: 16px;
  transform: translateX(-50%);
  color: rgba(217, 226, 255, 0.72);
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  font-weight: 700;
  line-height: 12px;
  white-space: nowrap;
  pointer-events: none;
}

.timeline-ruler-label.is-start {
  transform: none;
}

.timeline-ruler-label.is-end {
  transform: translateX(-100%);
}

.finished-playback-controls {
  background: linear-gradient(
    to top,
    rgba(3, 13, 37, 0.95),
    rgba(3, 13, 37, 0.4),
    transparent
  );
}

.finished-progress-track {
  height: 4px;
  border-radius: 999px;
  background: rgba(217, 226, 255, 0.15);
  overflow: hidden;
}

.modal-preview-progress {
  height: 4px;
  border-radius: 999px;
  background: rgba(217, 226, 255, 0.18);
  overflow: visible;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.22);
  touch-action: none;
  user-select: none;
}

.modal-preview-progress-thumb {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  border-radius: 999px;
  background: #ffffff;
  border: 2px solid #4a8eff;
  box-shadow: 0 0 12px rgba(74, 142, 255, 0.8);
  transform: translate(-50%, -50%);
  pointer-events: none;
}

.modal-preview-progress.is-dragging .modal-preview-progress-thumb {
  width: 14px;
  height: 14px;
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
  transition:
    width 0.25s ease,
    height 0.25s ease;
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

@keyframes spin {
  to {
    transform: rotate(360deg);
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

.start-editing-spinner {
  animation: spin 0.9s linear infinite;
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

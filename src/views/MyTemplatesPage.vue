<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import logoImage from '../assets/logo.png';
import AccountCenterMenu from '../components/AccountCenterMenu.vue';
import AppIcon from '../components/AppIcon.vue';
import { submitLocalTemplate } from '../api/templateUpload';
import { queryMyTemplates } from '../api/template';
import { systemMessage } from '../utils/message';

const router = useRouter();
const templates = ref([]);
const loading = ref(false);
const openingTemplateId = ref('');
const submittingTemplateKey = ref('');
const uploadDialogOpen = ref(false);
const uploadProgress = ref(0);
const uploadStage = ref('');
const uploadError = ref('');
const uploadComplete = ref(false);
const uploadTemplate = ref(null);

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
const TEMPLATE_STATUS_LABELS = Object.freeze({
  0: '已发布',
  1: '待发布',
  2: '编辑中',
});

function templateStatusLabel(status) {
  return TEMPLATE_STATUS_LABELS[Number(status)] || '编辑中';
}

function getTemplateQueryIdentity() {
  const stored = JSON.parse(localStorage.getItem('userInfo') || 'null') || {};
  const profile = stored.user || stored.profile || stored.sysUser || stored;
  const renterId = stored.renterId ?? stored.tenantId ??
    profile.renterId ?? profile.tenantId;
  const createBy = profile.userName ?? stored.userName;
  if (renterId === undefined || renterId === null || String(renterId) === '') {
    throw new Error('登录信息中缺少 renterId，无法同步模板状态');
  }
  if (createBy === undefined || createBy === null || String(createBy) === '') {
    throw new Error('登录信息中缺少 userName，无法同步模板状态');
  }
  return { renterId: String(renterId), createBy: String(createBy) };
}

function goHome() {
  router.push({ name: 'home' });
}

function createTemplate() {
  router.push({ name: 'create-template' });
}

function openProjectLibrary() {
  router.push({ name: 'home', query: { open: 'draft-library' } });
}

function handleAccountLogout() {
  localStorage.removeItem('token');
  localStorage.removeItem('userInfo');
  router.replace('/login');
}

function formatDuration(durationMs) {
  const totalSeconds = Math.max(0, Math.round(Number(durationMs) || 0) / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
}

function previewUrl(path) {
  if (!path) return '';
  try {
    return convertFileSrc(path);
  } catch {
    return '';
  }
}

function showVideoFirstFrame(event) {
  const video = event.currentTarget;
  if (!video || !Number.isFinite(video.duration) || video.duration <= 0) return;
  video.currentTime = Math.min(0.1, video.duration / 2);
}

async function loadTemplates() {
  if (loading.value) return;
  loading.value = true;
  try {
    const result = await invoke('list_custom_templates');
    templates.value = Array.isArray(result) ? result : [];
    try {
      const response = await queryMyTemplates(getTemplateQueryIdentity());
      if (!response || Number(response.code) !== 0 || !Array.isArray(response.rows)) {
        throw new Error(response?.msg || '查询模板状态失败');
      }
      const remoteTemplateById = new Map(
        response.rows
          .filter((row) => row?.templateId !== undefined && row?.templateId !== null)
          .map((row) => [String(row.templateId), row]),
      );
      await Promise.all(
        templates.value.map(async (template) => {
          const remoteTemplate = remoteTemplateById.get(
            String(template.backendTemplateId || ''),
          );
          if (Number(template.status) === 2) return;
          const remoteStatus = Number(remoteTemplate?.status);
          if (!remoteTemplate || ![0, 1, 2].includes(remoteStatus) ||
            remoteStatus === Number(template.status)) {
            return;
          }
          await invoke('update_custom_template_status', {
            templateId: template.templateId,
            status: remoteStatus,
          });
          template.status = remoteStatus;
        }),
      );
    } catch (error) {
      console.warn('[my-templates] 模板状态同步失败', error);
      systemMessage.error(error?.message || '模板状态同步失败，已显示本地状态');
    }
  } catch (error) {
    templates.value = [];
    systemMessage.error(error?.message || String(error) || '我的模板加载失败');
  } finally {
    loading.value = false;
  }
}

async function editTemplate(template) {
  if (!template?.templateId || openingTemplateId.value) return;
  openingTemplateId.value = String(template.templateId);
  try {
    await router.push({
      name: 'create-template',
      query: { customTemplateId: String(template.templateId) },
    });
  } finally {
    openingTemplateId.value = '';
  }
}

async function submitTemplate(template) {
  if (
    !template?.templateId ||
    !template.submissionReady ||
    Number(template.status) !== 2 ||
    submittingTemplateKey.value
  ) return;
  uploadTemplate.value = template;
  submittingTemplateKey.value = String(template.templateId);
  uploadDialogOpen.value = true;
  uploadProgress.value = 0;
  uploadStage.value = '正在准备上传...';
  uploadError.value = '';
  uploadComplete.value = false;
  try {
    await submitLocalTemplate(template, (update) => {
      uploadStage.value = update.stage || uploadStage.value;
      uploadProgress.value = Math.max(0, Math.min(100, Number(update.progress) || 0));
    });
    template.status = 1;
    template.submissionReady = false;
    uploadComplete.value = true;
    systemMessage.success('模板上传成功');
  } catch (error) {
    uploadError.value = error?.message || String(error) || '模板上传失败';
    uploadStage.value = '上传失败';
    systemMessage.error(uploadError.value);
  } finally {
    submittingTemplateKey.value = '';
  }
}

function closeUploadDialog() {
  if (submittingTemplateKey.value) return;
  uploadDialogOpen.value = false;
}

onMounted(() => {
  document.title = '我的模板 · 艾咔';
  void loadTemplates();
});
</script>

<template>
  <div class="my-templates-page">
    <header
      class="fixed top-0 left-0 right-0 z-[300] bg-surface-container backdrop-blur-2xl border-b border-primary/20"
    >
      <div
        class="flex items-center px-6 gap-4 overflow-visible no-scrollbar border-b-2 border-white/10 h-16"
      >
        <div class="flex items-center gap-4 shrink-0">
          <img
            alt="艾咔"
            class="h-10 w-auto object-contain cursor-pointer select-none"
            :src="logoImage"
            @click="goHome"
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
            <span class="text-[13px] text-electric-blue font-bold whitespace-nowrap">
              {{ accountTenantName }}
            </span>
          </div>
          <div class="flex items-center gap-1.5 px-3 py-1 rounded-full">
            <span class="text-[13px] text-on-surface-variant font-medium whitespace-nowrap">
              {{ accountDisplayName }}
            </span>
          </div>
        </div>
        <div class="flex-1"></div>
        <button class="header-action" type="button" @click="createTemplate">
          <span>创建模板</span>
        </button>
        <button class="header-action" type="button" @click="openProjectLibrary">
          <span>工程库</span>
        </button>
        <button class="header-action is-disabled" type="button" disabled>
          <span>导出</span>
        </button>
        <AccountCenterMenu @logout="handleAccountLogout" />
      </div>
    </header>

    <main class="app-shell">
      <section class="library-heading">
        <h2>我的模板</h2>
      </section>

      <section class="library-content">
        <div v-if="loading && !templates.length" class="library-state">
          <AppIcon name="progress_activity" :size="34" class="spinning" />
          <strong>正在解析我的模板...</strong>
        </div>

        <div v-else-if="!templates.length" class="library-state">
          <span class="state-icon"><AppIcon name="video_library" :size="38" /></span>
          <strong>还没有自己创建的模板</strong>
          <button type="button" @click="createTemplate">去创建模板</button>
        </div>

        <div v-else class="template-grid">
          <article
            v-for="template in templates"
            :key="template.templateId"
            class="template-card"
            :class="{ 'is-opening': Boolean(openingTemplateId) }"
            role="button"
            tabindex="0"
            @click="editTemplate(template)"
            @keydown.enter.self="editTemplate(template)"
            @keydown.space.prevent.self="editTemplate(template)"
          >
            <div class="template-preview">
              <video
                v-if="previewUrl(template.previewPath)"
                :src="previewUrl(template.previewPath)"
                muted
                playsinline
                preload="metadata"
                @loadedmetadata="showVideoFirstFrame"
              ></video>
              <div v-else class="template-preview-empty">
                <AppIcon name="movie" :size="42" />
              </div>
              <span class="duration-badge">{{ formatDuration(template.durationMs) }}</span>
              <span class="edit-mask">
                <AppIcon
                  :name="openingTemplateId === String(template.templateId) ? 'progress_activity' : 'edit'"
                  :size="22"
                  :class="{ spinning: openingTemplateId === String(template.templateId) }"
                />
                {{ openingTemplateId === String(template.templateId) ? '正在打开' : '继续编辑' }}
              </span>
            </div>
            <div class="template-info">
              <div class="template-title-row">
                <strong :title="template.name">{{ template.name }}</strong>
                <button
                  v-if="Number(template.status) === 2 && template.submissionReady"
                  class="submit-button"
                  type="button"
                  title="提交模板"
                  aria-label="提交模板"
                  :disabled="Boolean(submittingTemplateKey)"
                  @click.stop="submitTemplate(template)"
                >
                  <AppIcon
                    :name="submittingTemplateKey === String(template.templateId) ? 'progress_activity' : 'publish'"
                    :size="17"
                    :class="{ spinning: submittingTemplateKey === String(template.templateId) }"
                  />
                  <span>{{ submittingTemplateKey === String(template.templateId) ? '上传中' : '提交' }}</span>
                </button>
              </div>
              <div class="template-meta">
                <span
                  class="template-status"
                  :class="`is-status-${Number(template.status)}`"
                >
                  {{ templateStatusLabel(template.status) }}
                </span>
                <span><AppIcon name="aspect_ratio" :size="14" />{{ template.resolution || '--' }}</span>
                <span><AppIcon name="schedule" :size="14" />{{ template.updatedAt || '--' }}</span>
              </div>
            </div>
          </article>
        </div>
      </section>
    </main>
    <div v-if="uploadDialogOpen" class="upload-dialog-backdrop" role="presentation">
      <section class="upload-dialog" role="dialog" aria-modal="true" aria-labelledby="upload-dialog-title">
        <h2 id="upload-dialog-title">{{ uploadComplete ? '模板上传完成' : uploadError ? '模板上传失败' : '正在上传模板' }}</h2>
        <p class="upload-template-name" :title="uploadTemplate?.name">{{ uploadTemplate?.name }}</p>
        <p class="upload-stage">{{ uploadStage }}</p>
        <div class="upload-progress-heading"><span>总进度</span><span>{{ uploadProgress }}%</span></div>
        <div class="upload-progress-track"><div :style="{ width: `${uploadProgress}%` }"></div></div>
        <p v-if="uploadError" class="upload-error">{{ uploadError }}</p>
        <div v-if="!submittingTemplateKey" class="upload-dialog-actions">
          <button v-if="uploadError" type="button" class="upload-retry" @click="submitTemplate(uploadTemplate)">重试上传</button>
          <button type="button" class="upload-dismiss" @click="closeUploadDialog">{{ uploadComplete ? '完成' : '关闭' }}</button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.my-templates-page {
  min-height: 100vh;
  color: #20201e;
  background: #efefeb;
}

.header-action {
  width: 96px;
  height: 36px;
  flex: none;
  color: #aeb7cb;
  border: 1px solid rgba(142, 151, 170, 0.2);
  border-radius: 8px;
  background: rgba(17, 24, 43, 0.5);
  font-size: 13px;
  font-weight: 700;
  transition: 160ms ease;
}

.header-action:hover:not(:disabled) {
  color: #4a8eff;
  background: #1a2237;
}

.header-action.is-disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.app-shell {
  position: fixed;
  inset: 64px 0 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: #efefeb;
  font-family: 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
}

.library-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 40px;
  border-bottom: 1px solid #e3e3de;
  background: rgba(255, 255, 255, 0.92);
}

.library-heading h2 {
  margin: 0;
  font-size: 22px;
  font-weight: 900;
}

.library-state p {
  margin: 0;
  color: #777872;
  font-size: 13px;
}

.library-state button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 10px 18px;
  color: white;
  border-radius: 9px;
  background: #20201e;
  font-size: 13px;
  font-weight: 800;
}

.library-content {
  flex: 1;
  overflow: auto;
  padding: 30px 40px 48px;
}

.template-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 22px;
  max-width: 1500px;
  margin: 0 auto;
}

.template-card {
  min-width: 0;
  overflow: hidden;
  cursor: pointer;
  color: #20201e;
  border: 1px solid #dfdfd9;
  border-radius: 14px;
  background: white;
  box-shadow: 0 8px 26px rgba(32, 32, 30, 0.06);
  text-align: left;
  transition: 180ms ease;
}

.template-card:focus-visible {
  outline: 2px solid rgba(242, 102, 69, 0.75);
  outline-offset: 3px;
}

.template-card.is-opening {
  cursor: wait;
}

.template-card:hover:not(.is-opening) {
  border-color: rgba(242, 102, 69, 0.65);
  box-shadow: 0 14px 34px rgba(32, 32, 30, 0.12);
  transform: translateY(-3px);
}

.template-preview {
  position: relative;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  background: #20201e;
}

.template-preview video {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.template-preview-empty {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  color: rgba(255, 255, 255, 0.42);
  background: radial-gradient(circle at 50% 40%, #42433f, #20201e 68%);
}

.duration-badge {
  position: absolute;
  right: 10px;
  bottom: 10px;
  padding: 3px 7px;
  color: white;
  border-radius: 5px;
  background: rgba(0, 0, 0, 0.72);
  font-size: 11px;
  font-weight: 700;
}

.submit-button {
  width: 68px;
  height: 30px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin: 0;
  color: #f26645;
  border: 1px solid rgba(242, 102, 69, 0.34);
  border-radius: 8px;
  background: #fff0eb;
  font-size: 12px;
  font-weight: 800;
  transition: 160ms ease;
}

.submit-button:hover {
  color: white;
  background: #f26645;
  transform: translateY(-1px);
}

.submit-button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.upload-dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 500;
  display: grid;
  place-items: center;
  background: rgba(17, 20, 23, 0.62);
}

.upload-dialog {
  width: min(440px, calc(100vw - 32px));
  padding: 24px;
  color: #20201e;
  border-radius: 16px;
  background: #fff;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.25);
}

.upload-dialog h2 {
  margin: 0;
  font-size: 20px;
}

.upload-template-name {
  margin: 6px 0 18px;
  overflow: hidden;
  color: #777872;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.upload-stage {
  min-height: 20px;
  margin: 0 0 15px;
  font-weight: 700;
}

.upload-progress-heading {
  display: flex;
  justify-content: space-between;
  margin: 14px 0 6px;
  font-size: 12px;
  font-weight: 700;
}

.upload-progress-track {
  height: 9px;
  overflow: hidden;
  border-radius: 20px;
  background: #eee;
}

.upload-progress-track > div {
  height: 100%;
  border-radius: inherit;
  background: #f26645;
  transition: width 220ms ease;
}

.upload-error {
  margin: 16px 0 0;
  color: #c73b32;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.upload-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}

.upload-dialog-actions button {
  padding: 9px 15px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 800;
}

.upload-retry {
  color: white;
  background: #f26645;
}

.upload-dismiss {
  color: #333;
  background: #ededeb;
}

.edit-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: white;
  background: rgba(20, 20, 19, 0.62);
  font-size: 14px;
  font-weight: 900;
  opacity: 0;
  transition: 180ms ease;
}

.template-card:hover .edit-mask,
.template-card.is-opening .edit-mask {
  opacity: 1;
}

.template-info {
  padding: 15px 16px 17px;
}

.template-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.template-title-row > strong {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 15px;
  font-weight: 900;
}

.template-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 7px 12px;
  margin-top: 13px;
  padding-top: 11px;
  color: #777872;
  border-top: 1px solid #efefeb;
  font-size: 11px;
}

.template-meta span {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.template-meta .template-status {
  padding: 2px 7px;
  border-radius: 5px;
  color: #a34e28;
  background: #fff0e6;
  font-weight: 800;
  white-space: nowrap;
}

.template-meta .template-status.is-status-0 {
  color: #287a51;
  background: #e3f5e9;
}

.template-meta .template-status.is-status-1 {
  color: #7d6517;
  background: #fff4cb;
}

.library-state {
  min-height: 420px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: #777872;
}

.library-state strong {
  color: #20201e;
  font-size: 18px;
}

.library-state button {
  margin-top: 8px;
  background: #f26645;
}

.state-icon {
  width: 72px;
  height: 72px;
  display: grid;
  place-items: center;
  color: #f26645;
  border-radius: 18px;
  background: #fff0eb;
}

.spinning {
  animation: template-spin 0.8s linear infinite;
}

@keyframes template-spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 900px) {
  .library-heading,
  .library-content {
    padding-right: 22px;
    padding-left: 22px;
  }

  .template-grid {
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  }
}
</style>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { useRouter } from 'vue-router';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { getUserInfo, logoutUser, resetPassword } from '../api/user';
import { systemMessage } from '../utils/message';
import AppIcon from './AppIcon.vue';

const props = defineProps({
  refreshKey: {
    type: Number,
    default: 0,
  },
});
const emit = defineEmits(['logout']);
const router = useRouter();

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '';
const menuVisible = ref(false);
const profileVisible = ref(false);
const passwordVisible = ref(false);
const helpVisible = ref(false);
const logoutVisible = ref(false);
const activeHelpTab = ref('guide');
const profileRefreshing = ref(false);
const passwordSubmitting = ref(false);
const helpGuideDownloading = ref(false);
const logoutSubmitting = ref(false);
const profileRevision = ref(0);
const passwordForm = reactive({
  oldPassword: '',
  newPassword: '',
  confirmPassword: '',
});

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

const storedUserProfile = computed(() => {
  props.refreshKey;
  profileRevision.value;
  return getStoredUserProfile();
});
const accountDisplayName = computed(() => {
  const profile = storedUserProfile.value;
  return profile.phone || profile.phonenumber || '--';
});
const accountVersionName = computed(
  () => storedUserProfile.value.nickName || '--',
);
const accountTenantName = computed(() => {
  const profile = storedUserProfile.value;
  return profile.renterName || profile.tenantName || '--';
});
const accountBalance = computed(() => {
  const balance = Number(storedUserProfile.value.creditBalance);
  if (!Number.isFinite(balance)) return '--';
  return new Intl.NumberFormat('zh-CN', {
    maximumFractionDigits: 2,
  }).format(balance);
});

function closeMenu() {
  menuVisible.value = false;
}

function toggleMenu() {
  menuVisible.value = !menuVisible.value;
}

function normalizeUserInfoPayload(response) {
  const payload = response?.data || response || {};
  const user =
    payload.user || payload.profile || payload.sysUser || response?.user || payload;
  return {
    ...user,
    userId: user.userId || payload.userId || '',
    tenantId:
      user.tenantId || user.renterId || payload.tenantId || payload.renterId || '',
    renterId:
      user.renterId || user.tenantId || payload.renterId || payload.tenantId || '',
    phone: user.phone || user.phonenumber || payload.phone || '',
    roles: response?.roles || payload.roles || [],
    permissions: response?.permissions || payload.permissions || [],
  };
}

function syncStoredUserInfo(nextProfile) {
  if (!nextProfile || typeof nextProfile !== 'object') return;
  const current = getStoredUserInfo();
  const merged = { ...current, ...nextProfile };
  for (const key of ['user', 'profile', 'sysUser']) {
    if (current[key] && typeof current[key] === 'object') {
      merged[key] = { ...current[key], ...nextProfile };
    }
  }
  localStorage.setItem('userInfo', JSON.stringify(merged));
  profileRevision.value += 1;
}

async function refreshUserInfo() {
  if (profileRefreshing.value) return;
  const stored = getStoredUserInfo();
  profileRefreshing.value = true;
  try {
    const response = await getUserInfo({
      renterId: stored.renterId || stored.tenantId || '',
      userId: stored.userId || '',
    });
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '用户信息查询失败');
    }
    syncStoredUserInfo(normalizeUserInfoPayload(response));
  } catch (error) {
    systemMessage.error(error?.message || '用户信息刷新失败');
  } finally {
    profileRefreshing.value = false;
  }
}

function showProfile() {
  closeMenu();
  profileVisible.value = true;
  refreshUserInfo();
}

function resetPasswordForm() {
  passwordForm.oldPassword = '';
  passwordForm.newPassword = '';
  passwordForm.confirmPassword = '';
}

function showPassword() {
  closeMenu();
  passwordVisible.value = true;
}

function hidePassword() {
  if (passwordSubmitting.value) return;
  passwordVisible.value = false;
  resetPasswordForm();
}

async function submitPasswordReset() {
  if (passwordSubmitting.value) return;
  const oldPassword = passwordForm.oldPassword.trim();
  const newPassword = passwordForm.newPassword.trim();
  const confirmPassword = passwordForm.confirmPassword.trim();
  if (!oldPassword) return systemMessage.error('请输入旧密码');
  if (!newPassword) return systemMessage.error('请输入新密码');
  if (!confirmPassword) return systemMessage.error('请确认新密码');
  if (newPassword !== confirmPassword) {
    return systemMessage.error('两次输入的新密码不一致');
  }

  const stored = getStoredUserInfo();
  const profile = getStoredUserProfile();
  const renterId = stored.renterId || stored.tenantId || '';
  const userId = stored.userId || '';
  const phone = profile.phone || profile.phonenumber || '';
  if (!renterId || !userId || !phone) {
    return systemMessage.error('用户信息不完整，无法修改密码');
  }

  passwordSubmitting.value = true;
  try {
    const response = await resetPassword({
      renterId,
      userId: Number.isFinite(Number(userId)) ? Number(userId) : userId,
      phone,
      modifyType: 0,
      oldPassword,
      newPassword,
    });
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '修改密码失败');
    }
    systemMessage.success(response?.msg || '修改密码成功，请重新登录');
    passwordVisible.value = false;
    resetPasswordForm();
    emit('logout');
  } catch (error) {
    systemMessage.error(error?.message || '修改密码失败');
  } finally {
    passwordSubmitting.value = false;
  }
}

function showHelp() {
  closeMenu();
  helpVisible.value = true;
}

function showMyTemplates() {
  closeMenu();
  router.push({ name: 'my-templates' });
}

async function downloadHelpGuide() {
  if (helpGuideDownloading.value) return;
  helpGuideDownloading.value = true;
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: '选择指南保存目录',
    });
    const outputDir = Array.isArray(selected) ? selected[0] : selected;
    if (!outputDir) return;
    await invoke('download_help_guide', {
      apiBaseUrl: API_BASE_URL,
      authorizationToken: localStorage.getItem('token') || '',
      outputDir,
    });
    systemMessage.success('指南下载成功');
  } catch (error) {
    systemMessage.error(error?.message || String(error || '指南下载失败'));
  } finally {
    helpGuideDownloading.value = false;
  }
}

function showLogout() {
  closeMenu();
  logoutVisible.value = true;
}

async function confirmLogout() {
  if (logoutSubmitting.value) return;
  logoutSubmitting.value = true;
  try {
    const response = await logoutUser();
    if (response?.code !== undefined && Number(response.code) !== 0) {
      throw new Error(response?.msg || '退出登录失败');
    }
    logoutVisible.value = false;
    emit('logout');
  } catch (error) {
    systemMessage.error(error?.message || '退出登录失败');
  } finally {
    logoutSubmitting.value = false;
  }
}

onMounted(() => document.addEventListener('pointerdown', closeMenu));
onBeforeUnmount(() => document.removeEventListener('pointerdown', closeMenu));
</script>

<template>
  <div class="relative z-[330]" @pointerdown.stop>
    <button
      class="h-9 w-24 shrink-0 flex items-center justify-center gap-1.5 bg-surface-container-low/50 text-on-surface-variant shadow-sm hover:bg-surface-container-high hover:text-electric-blue focus:bg-electric-blue focus:text-white rounded-lg transition-all focus:outline-none active:scale-95 border border-outline-variant/20"
      type="button"
      :aria-expanded="menuVisible"
      aria-haspopup="menu"
      @click="toggleMenu"
    >
      <span class="text-[13px] font-bold">个人中心</span>
    </button>
    <div
      class="absolute top-full right-0 mt-2 w-36 bg-surface-container-highest/95 backdrop-blur-xl border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.5)] transition-all duration-200 ease-out z-[400] py-2 overflow-hidden"
      :class="
        menuVisible
          ? 'opacity-100 translate-y-0 pointer-events-auto'
          : 'opacity-0 translate-y-2 pointer-events-none'
      "
      role="menu"
    >
      <button class="account-menu-item" type="button" @click="showProfile">
        <AppIcon name="account_circle" :size="18" /><span>个人信息</span>
      </button>
      <div class="account-menu-divider"></div>
      <button class="account-menu-item" type="button" @click="showPassword">
        <AppIcon name="lock_reset" :size="18" /><span>修改密码</span>
      </button>
      <div class="account-menu-divider"></div>
      <button class="account-menu-item" type="button" @click="showMyTemplates">
        <AppIcon name="video_library" :size="18" /><span>我的模板</span>
      </button>
      <div class="account-menu-divider"></div>
      <button class="account-menu-item" type="button" @click="showHelp">
        <AppIcon name="help_center" :size="18" /><span>帮助中心</span>
      </button>
      <div class="account-menu-divider"></div>
      <button
        class="account-menu-item account-menu-logout"
        type="button"
        @click="showLogout"
      >
        <AppIcon name="logout" :size="18" /><span>退出登录</span>
      </button>
    </div>
  </div>

  <Teleport to="body">
    <div
      v-if="helpVisible"
      class="fixed inset-0 z-[440] flex items-center justify-center"
    >
      <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" @click="helpVisible = false"></div>
      <div class="account-large-modal">
        <header class="account-modal-header">
          <div class="flex items-center gap-3">
            <span class="account-modal-icon"><AppIcon name="menu_book" :size="20" /></span>
            <h3>产品帮助中心</h3>
          </div>
          <button type="button" @click="helpVisible = false"><AppIcon name="close" :size="24" /></button>
        </header>
        <div class="flex-1 flex overflow-hidden">
          <aside class="account-help-nav">
            <button v-for="tab in [{ id: 'guide', icon: 'explore', label: '使用指南' }, { id: 'faq', icon: 'quiz', label: '常见问题' }, { id: 'changelog', icon: 'history', label: '更新日志' }]" :key="tab.id" type="button" :class="{ active: activeHelpTab === tab.id }" @click="activeHelpTab = tab.id">
              <AppIcon :name="tab.icon" :size="20" /><span>{{ tab.label }}</span>
            </button>
          </aside>
          <main class="account-help-content">
            <div v-if="activeHelpTab === 'guide'" class="account-help-copy">
              <h1>核心使用指南 V1.0</h1>
              <p>欢迎使用 AICut 专业版剪辑系统。选择模板后可按素材位导入视频，进入编辑页面调整画面，并导出最终成片。</p>
              <h4>智能模板匹配</h4>
              <p>系统会按模板素材位展示所需视频数量和时长范围。</p>
              <h4>一键批量导入</h4>
              <p>可以按分类批量替换素材，也可以单独替换某一个素材位。</p>
            </div>
            <div v-else-if="activeHelpTab === 'faq'" class="account-help-copy">
              <h1>常见问题</h1>
              <h4>导入素材后为什么不能导出？</h4>
              <p>需要先点击开始编辑，生成工程文件后导出按钮才会可用。</p>
              <h4>时间滑块有什么作用？</h4>
              <p>时间滑块用于选择素材在原视频中的起始片段，工程文件会同步记录偏移时间。</p>
            </div>
            <div v-else class="account-help-copy">
              <h1>更新日志</h1>
              <h4>当前版本</h4>
              <p>优化模板素材导入、时间轴偏移、工程创建和导出流程。</p>
            </div>
            <button class="account-help-download" type="button" :disabled="helpGuideDownloading" @click="downloadHelpGuide">
              <AppIcon :name="helpGuideDownloading ? 'progress_activity' : 'download'" :size="22" :class="{ spinning: helpGuideDownloading }" />
              {{ helpGuideDownloading ? '下载中...' : '下载离线指南' }}
            </button>
          </main>
        </div>
      </div>
    </div>

    <div v-if="profileVisible" class="account-modal-layer z-[430]">
      <div class="account-modal-shade" @click="profileVisible = false"></div>
      <section class="account-profile-modal">
        <button class="account-modal-close" type="button" @click="profileVisible = false"><AppIcon name="close" :size="24" /></button>
        <span class="account-profile-icon"><AppIcon name="person" :size="30" /></span>
        <h3>个人信息</h3>
        <div class="account-profile-list">
          <div><span>账号</span><strong>{{ accountDisplayName }}</strong></div>
          <div><span>昵称</span><strong>{{ accountVersionName }}</strong></div>
          <div><span>所属租户</span><strong class="text-electric-blue">{{ accountTenantName }}</strong></div>
          <div><span>剩余积分</span><strong class="account-balance">{{ accountBalance }} <small>pts</small></strong></div>
        </div>
        <button class="account-close-button" type="button" @click="profileVisible = false">关闭</button>
      </section>
    </div>

    <div v-if="passwordVisible" class="account-modal-layer z-[430]">
      <div class="account-modal-shade" @click="hidePassword"></div>
      <section class="account-form-modal">
        <h3><AppIcon name="lock_reset" :size="24" />修改密码</h3>
        <input v-model="passwordForm.oldPassword" :disabled="passwordSubmitting" placeholder="旧密码" type="password" @keydown.enter.prevent="submitPasswordReset" />
        <input v-model="passwordForm.newPassword" :disabled="passwordSubmitting" placeholder="新密码" type="password" @keydown.enter.prevent="submitPasswordReset" />
        <input v-model="passwordForm.confirmPassword" :disabled="passwordSubmitting" placeholder="确认密码" type="password" @keydown.enter.prevent="submitPasswordReset" />
        <button class="account-primary-button" type="button" :disabled="passwordSubmitting" @click="submitPasswordReset">{{ passwordSubmitting ? '正在保存...' : '保存修改' }}</button>
        <button class="account-close-button" type="button" :disabled="passwordSubmitting" @click="hidePassword">取消</button>
      </section>
    </div>

    <div v-if="logoutVisible" class="account-modal-layer z-[450]">
      <div class="account-modal-shade" @click="logoutVisible = false"></div>
      <section class="account-form-modal text-center">
        <span class="account-logout-icon"><AppIcon name="logout" :size="30" /></span>
        <h3 class="justify-center">退出登录</h3>
        <p>确认退出当前账号吗？</p>
        <button class="account-danger-button" type="button" :disabled="logoutSubmitting" @click="confirmLogout">{{ logoutSubmitting ? '正在退出...' : '确认退出' }}</button>
        <button class="account-close-button" type="button" :disabled="logoutSubmitting" @click="logoutVisible = false">取消</button>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.account-menu-item { width: 100%; display: flex; align-items: center; gap: 0.75rem; padding: 0.625rem 1rem; color: #aeb7cb; font-size: 13px; text-align: left; transition: 160ms ease; }
.account-menu-item:hover { color: white; background: rgba(74, 142, 255, 0.1); }
.account-menu-divider { margin: 0.25rem 0.5rem; border-top: 1px solid rgba(142, 151, 170, 0.3); }
.account-menu-logout { color: #ec4034; }
.account-menu-logout:hover { color: #ec4034; background: rgba(236, 64, 52, 0.1); }
.account-modal-layer { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; }
.account-modal-shade { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); }
.account-large-modal { position: relative; width: 90vw; height: 85vh; display: flex; flex-direction: column; overflow: hidden; color: white; border: 1px solid rgba(255,255,255,.1); border-radius: 24px; background: #11182b; box-shadow: 0 32px 64px -12px rgba(0,0,0,.8); }
.account-modal-header { height: 64px; display: flex; flex: none; align-items: center; justify-content: space-between; padding: 0 32px; border-bottom: 1px solid rgba(255,255,255,.1); background: #1a2237; }
.account-modal-header h3, .account-form-modal h3, .account-profile-modal h3 { margin: 0; color: white; font-size: 18px; font-weight: 900; }
.account-modal-header > button, .account-modal-close { color: #aeb7cb; }
.account-modal-icon { width: 32px; height: 32px; display: grid; place-items: center; border-radius: 8px; background: #4a8eff; }
.account-help-nav { width: 256px; flex: none; padding: 16px; border-right: 1px solid rgba(255,255,255,.05); background: rgba(0,0,0,.2); }
.account-help-nav button { width: 100%; display: flex; align-items: center; gap: 12px; padding: 12px 16px; color: #aeb7cb; border-radius: 12px; text-align: left; }
.account-help-nav button:hover { background: rgba(255,255,255,.05); }
.account-help-nav button.active { color: #4a8eff; background: rgba(74,142,255,.1); font-weight: 700; }
.account-help-content { position: relative; flex: 1; overflow: auto; padding: 48px; background: #071023; }
.account-help-copy { max-width: 720px; margin: 0 auto; }
.account-help-copy h1 { margin: 0 0 24px; color: white; font-size: 30px; font-weight: 900; }
.account-help-copy h4 { margin: 24px 0 8px; color: #4a8eff; font-weight: 700; }
.account-help-copy p, .account-form-modal p { color: #aeb7cb; font-size: 13px; line-height: 1.7; }
.account-help-download { position: absolute; right: 48px; bottom: 48px; display: flex; align-items: center; gap: 8px; padding: 12px 24px; color: white; border-radius: 999px; background: #4a8eff; font-weight: 900; }
.account-profile-modal, .account-form-modal { position: relative; width: min(448px, calc(100vw - 32px)); padding: 32px; color: white; border: 1px solid rgba(255,255,255,.1); border-radius: 16px; background: #1a2237; box-shadow: 0 24px 60px rgba(0,0,0,.5); }
.account-modal-close { position: absolute; top: 16px; right: 16px; }
.account-profile-icon, .account-logout-icon { width: 64px; height: 64px; display: grid; margin: 0 auto 16px; place-items: center; border-radius: 50%; color: #4a8eff; background: rgba(74,142,255,.1); }
.account-profile-modal > h3 { margin-bottom: 24px; text-align: center; }
.account-profile-list { padding: 20px; border: 1px solid rgba(255,255,255,.05); border-radius: 12px; background: rgba(0,0,0,.2); }
.account-profile-list > div { display: flex; justify-content: space-between; gap: 16px; padding: 7px 0; font-size: 12px; }
.account-profile-list span { color: #78839b; }
.account-profile-list strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-balance { color: #4a8eff; font-size: 22px; }
.account-balance small { font-size: 10px; }
.account-form-modal { width: min(384px, calc(100vw - 32px)); display: flex; flex-direction: column; gap: 12px; }
.account-form-modal h3 { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
.account-form-modal input { width: 100%; padding: 12px 16px; color: white; border: 1px solid rgba(255,255,255,.1); border-radius: 12px; outline: none; background: #11182b; }
.account-form-modal input:focus { border-color: rgba(74,142,255,.6); }
.account-primary-button, .account-danger-button, .account-close-button { width: 100%; padding: 12px; border-radius: 12px; font-weight: 700; }
.account-primary-button { color: white; background: #4a8eff; }
.account-danger-button { color: white; background: #ec4034; }
.account-close-button { color: #aeb7cb; background: rgba(255,255,255,.05); }
.account-logout-icon { color: #ec4034; background: rgba(236,64,52,.08); border: 1px solid rgba(255,255,255,.1); }
.spinning { animation: account-spin .8s linear infinite; }
@keyframes account-spin { to { transform: rotate(360deg); } }
@media (max-width: 700px) { .account-help-nav { width: 170px; } .account-help-content { padding: 24px; } }
</style>

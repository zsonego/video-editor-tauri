<script setup>
import { computed, onMounted, ref } from 'vue';
import { getUserInfo, loginUser } from '../api/user';
import { systemMessage } from '../utils/message';
import bgImage from '../assets/bg.png';
import logoImage from '../assets/logo1.png';

const emit = defineEmits(['login']);

const account = ref('');
const password = ref('');
const passwordVisible = ref(false);
const submitting = ref(false);

const tenantDialogVisible = ref(false);
const tenantList = ref([]);
const selectedTenantId = ref('');

const hasTenants = computed(() => tenantList.value.length > 0);

function getLoginIdentity(loginResponse, payload) {
  const tenants = Array.isArray(loginResponse?.tenantList)
    ? loginResponse.tenantList
    : [];
  const selectedTenant =
    tenants.find((tenant) => tenant.tenantId === payload.tenantId) ||
    tenants[0] ||
    {};

  return {
    tenantId:
      payload.tenantId ||
      selectedTenant.tenantId ||
      loginResponse?.tenantId ||
      '',
    userId: selectedTenant.userId || loginResponse?.userId || '',
    tenantName: selectedTenant.tenantName || loginResponse?.tenantName || '',
  };
}

function saveLoginState(identity, userInfo) {
  const storedUserInfo = {
    ...userInfo,
    userId: userInfo?.userId || identity.userId,
    tenantId: userInfo?.tenantId || identity.tenantId,
    tenantName: userInfo?.tenantName || identity.tenantName,
  };
  const loginState = {
    ...identity,
    phone: storedUserInfo.phone || account.value.trim(),
    userInfo: storedUserInfo,
  };

  localStorage.setItem('loginState', JSON.stringify(loginState));
  localStorage.setItem('userInfo', JSON.stringify(storedUserInfo));
}

function togglePasswordVisible() {
  passwordVisible.value = !passwordVisible.value;
}

function handleBack() {
  if (window.history.length > 1) {
    window.history.back();
  }
}

function openTenantDialog(list) {
  tenantList.value = Array.isArray(list) ? list : [];
  selectedTenantId.value = tenantList.value[0]?.tenantId || '';
  tenantDialogVisible.value = true;
}

function closeTenantDialog() {
  tenantDialogVisible.value = false;
}

async function submitLogin(extra = {}) {
  const phone = account.value.trim();
  const loginPassword = password.value;

  if (!phone) {
    systemMessage.error('请输入账号');
    return;
  }

  if (!loginPassword) {
    systemMessage.error('请输入密码');
    return;
  }

  submitting.value = true;

  try {
    const payload = {
      phone,
      password: loginPassword,
      ...extra,
    };

    if (!payload.tenantId) {
      delete payload.tenantId;
    }

    const response = await loginUser(payload);
    const result = Number(response?.result);
    const backendMessage = response?.message || '';

    if (result === 0) {
      const identity = getLoginIdentity(response, payload);

      if (!identity.tenantId || !identity.userId) {
        systemMessage.error('登录成功，但未获取到用户身份信息');
        return;
      }

      const userInfoResponse = await getUserInfo({
        tenantId: identity.tenantId,
        userId: identity.userId,
      });
      const userInfo = userInfoResponse?.data || userInfoResponse;

      saveLoginState(identity, userInfo);
      systemMessage.success(backendMessage || '登录成功');
      tenantDialogVisible.value = false;
      emit('login', {
        login: response,
        userInfo,
        identity,
      });
      return;
    }

    if (result === 1) {
      const list = Array.isArray(response?.tenantList)
        ? response.tenantList
        : [];
      if (!list.length) {
        systemMessage.error(
          backendMessage || '当前账号存在多个租户，请选择后重试',
        );
        return;
      }
      openTenantDialog(list);
      return;
    }

    if (result === 2) {
      systemMessage.error('用户已绑定 2 个电脑');
      return;
    }

    systemMessage.error(backendMessage || '登录失败');
  } catch (error) {
    systemMessage.error(error?.message || '登录请求失败');
  } finally {
    submitting.value = false;
  }
}

function handleLogin() {
  submitLogin();
}

function selectTenant(tenantId) {
  selectedTenantId.value = tenantId;
}

function confirmTenantSelection() {
  if (!selectedTenantId.value) {
    systemMessage.error('请选择一个租户');
    return;
  }

  tenantDialogVisible.value = false;
  submitLogin({ tenantId: selectedTenantId.value });
}

onMounted(() => {
  document.title = '登录 - 鉴音';
});
</script>

<template>
  <section class="login-page" :style="{ backgroundImage: `url(${bgImage})` }">
    <main class="login-container">
      <div class="intro-panel">
        <img :src="logoImage" alt="鉴音" class="app-logo" />
        <div class="brand-copy">
          <h1>艾咔·专业版</h1>
          <p>释放无限创意，成就专业视界。专为高级视频创作者打造的精密工具。</p>
        </div>

        <div class="feature-list">
        <div class="feature-item">
          <div class="feature-icon">
            <span class="icon-text">4K</span>
          </div>
          <span>走向工业化标准的视频编辑器、零编辑经验也能秒出专业级大片</span>
        </div>

        <div class="feature-item">
          <div class="feature-icon">
            <svg class="feature-svg" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3 3 8l9 5 9-5-9-5Z" />
              <path d="m5 12 7 4 7-4" />
              <path d="m5 16 7 4 7-4" />
            </svg>
          </div>
          <span>模板化工作流 + 智能自动化</span>
        </div>

        <div class="feature-item">
          <div class="feature-icon">
            <svg class="feature-svg" viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="M17.5 18H8a5 5 0 1 1 1-9.9 6 6 0 0 1 11.2 3A3.5 3.5 0 0 1 17.5 18Z"
              />
              <path d="m14 14 2 2 2-2" />
              <path d="M16 12v4" />
            </svg>
          </div>
          <span>专为“快剪场景”打造的傻瓜型视频编辑器</span>
        </div>
        </div>
      </div>

      <div class="login-card-wrap">
        <div class="login-card">
          <button
            type="button"
            class="back-button"
            aria-label="返回"
            @click="handleBack"
          >
            <svg
              class="back-icon"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden="true"
            >
              <path
                d="M10 19l-7-7m0 0l7-7m-7 7h18"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>

          <div class="card-content">
            <div class="profile-icon">
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M20 21a8 8 0 0 0-16 0"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                />
                <circle
                  cx="12"
                  cy="7"
                  r="4"
                  stroke="currentColor"
                  stroke-width="2"
                />
              </svg>
            </div>

            <h2>账号登录</h2>

            <form class="login-form" @submit.prevent="handleLogin">
              <div class="form-row">
                <label for="login-account">账号</label>
                <input
                  id="login-account"
                  v-model="account"
                  class="input-underline"
                  placeholder="请输入手机号"
                  type="tel"
                  autocomplete="username"
                />
              </div>

              <div class="form-row password-row">
                <label for="login-password">密码</label>
                <input
                  id="login-password"
                  v-model="password"
                  class="input-underline password-input"
                  placeholder="请输入密码"
                  :type="passwordVisible ? 'text' : 'password'"
                  autocomplete="current-password"
                />
                <button
                  type="button"
                  class="toggle-password"
                  :aria-label="passwordVisible ? '隐藏密码' : '显示密码'"
                  @click="togglePasswordVisible"
                >
                  <svg
                    v-if="passwordVisible"
                    viewBox="0 0 24 24"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <circle
                      cx="12"
                      cy="12"
                      r="3"
                      stroke="currentColor"
                      stroke-width="2"
                    />
                  </svg>
                  <svg
                    v-else
                    viewBox="0 0 24 24"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path
                      d="M3 3l18 18"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M10.6 10.7A3 3 0 0 0 13.3 14"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M9.9 5.2A10.6 10.6 0 0 1 12 5c6.5 0 10 7 10 7a18 18 0 0 1-3.1 4.1"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                    <path
                      d="M6.7 6.7C3.7 8.7 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.2-1.5"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                    />
                  </svg>
                </button>
              </div>

              <div class="submit-wrap">
                <button
                  class="login-button"
                  type="submit"
                  :disabled="submitting"
                >
                  {{ submitting ? '登录中...' : '登录' }}
                </button>
              </div>
            </form>
          </div>
        </div>
      </div>
    </main>

    <transition name="tenant-fade">
      <div
        v-if="tenantDialogVisible"
        class="tenant-mask"
        @click.self="closeTenantDialog"
      >
        <div class="tenant-dialog">
          <div class="tenant-dialog-header">
            <div>
              <h3>选择租户</h3>
              <p>该账号关联了多个租户，请选择后继续登录。</p>
            </div>
            <button
              type="button"
              class="dialog-close"
              aria-label="关闭"
              @click="closeTenantDialog"
            >
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M6 6l12 12M18 6 6 18"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                />
              </svg>
            </button>
          </div>

          <div class="tenant-list">
            <button
              v-for="tenant in tenantList"
              :key="tenant.tenantId"
              type="button"
              class="tenant-item"
              :class="{ selected: selectedTenantId === tenant.tenantId }"
              @click="selectTenant(tenant.tenantId)"
            >
              <div class="tenant-main">
                <div class="tenant-name">
                  {{ tenant.tenantName || tenant.tenantId }}
                </div>
                <div class="tenant-id">{{ tenant.tenantId }}</div>
              </div>
              <div class="tenant-meta">
                <span
                  class="tenant-radio"
                  :class="{ checked: selectedTenantId === tenant.tenantId }"
                ></span>
              </div>
            </button>
          </div>

          <div class="tenant-actions">
            <button
              type="button"
              class="tenant-secondary"
              @click="closeTenantDialog"
            >
              取消
            </button>
            <button
              type="button"
              class="tenant-primary"
              :disabled="!hasTenants"
              @click="confirmTenantSelection"
            >
              确认登录
            </button>
          </div>
        </div>
      </div>
    </transition>
  </section>
</template>

<style scoped>
.login-page {
  width: 100%;
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  padding: 16px;
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  font-family:
    Inter,
    ui-sans-serif,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    sans-serif;
}

.login-container {
  width: 100%;
  max-width: 1240px;
  height: calc(100vh - 32px);
  max-height: 820px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 32px;
}

.intro-panel {
  width: 50%;
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 40px;
  color: #fff;
}

.app-logo {
  width: min(320px, 80vw);
  height: auto;
  object-fit: contain;
}

.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.brand-copy h1 {
  margin: 0;
  color: #fff;
  font-size: 56px;
  line-height: 1;
  font-weight: 700;
  letter-spacing: 0;
}

.brand-copy p {
  max-width: 440px;
  margin: 0;
  color: rgba(255, 255, 255, 0.82);
  font-size: 18px;
  line-height: 1.7;
}

.feature-list {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding-top: 20px;
}

.feature-item {
  display: flex;
  align-items: center;
  gap: 16px;
  color: rgba(255, 255, 255, 0.9);
  font-size: 18px;
  line-height: 1.5;
  font-weight: 500;
}

.feature-icon {
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  transition: background 0.2s ease;
}

.feature-item:hover .feature-icon {
  background: rgba(255, 255, 255, 0.2);
}

.icon-text {
  font-size: 20px;
  line-height: 1;
  font-weight: 700;
}

.feature-svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
}

.login-card-wrap {
  width: 440px;
  flex: 0 0 auto;
}

.login-card {
  position: relative;
  padding: 48px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 32px;
  background: rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(24px);
  -webkit-backdrop-filter: blur(24px);
}

.back-button {
  position: absolute;
  top: 24px;
  left: 24px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
}

.back-icon {
  width: 24px;
  height: 24px;
}

.card-content {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.profile-icon {
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 28px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 999px;
  background: rgba(26, 35, 126, 0.55);
}

.profile-icon svg {
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.92);
}

.card-content h2 {
  margin: 0 0 36px;
  color: rgba(255, 255, 255, 0.74);
  font-size: 12px;
  line-height: 1.4;
  font-weight: 600;
  letter-spacing: 0.2em;
  text-transform: uppercase;
}

.login-form {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.form-row {
  position: relative;
  display: flex;
  align-items: flex-end;
  gap: 12px;
}

.form-row label {
  flex: 0 0 auto;
  padding-bottom: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  line-height: 1.4;
  font-weight: 600;
}

.input-underline {
  width: 100%;
  min-width: 0;
  padding: 0 0 8px;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.28);
  border-radius: 0;
  background: transparent;
  color: #fff;
  font: inherit;
  font-size: 14px;
  line-height: 1.4;
  outline: none;
  box-shadow: none;
}

.input-underline::placeholder {
  color: rgba(255, 255, 255, 0.42);
}

.input-underline:focus {
  border-bottom-color: #fff;
}

.password-input {
  padding-right: 40px;
}

.toggle-password {
  position: absolute;
  right: 0;
  bottom: 8px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.42);
  cursor: pointer;
}

.toggle-password svg {
  width: 20px;
  height: 20px;
}

.submit-wrap {
  padding-top: 18px;
}

.login-button,
.tenant-primary,
.tenant-secondary {
  border: 0;
  font: inherit;
  cursor: pointer;
}

.login-button {
  width: 100%;
  padding: 15px 16px;
  border-radius: 14px;
  background: #152b94;
  color: #fff;
  font-weight: 600;
  box-shadow: 0 0 20px rgba(21, 43, 148, 0.35);
  transition:
    transform 0.2s ease,
    background 0.2s ease;
}

.login-button:hover:not(:disabled) {
  background: #1e40af;
  transform: translateY(-1px);
}

.login-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.tenant-mask {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: rgba(2, 6, 23, 0.62);
  backdrop-filter: blur(8px);
}

.tenant-dialog {
  width: min(520px, 100%);
  padding: 22px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 24px;
  background: rgba(7, 18, 42, 0.96);
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
}

.tenant-dialog-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.tenant-dialog-header h3 {
  margin: 0;
  color: #fff;
  font-size: 18px;
  line-height: 1.3;
}

.tenant-dialog-header p {
  margin: 6px 0 0;
  color: rgba(217, 226, 255, 0.62);
  font-size: 13px;
  line-height: 1.5;
}

.dialog-close {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.72);
}

.dialog-close svg {
  width: 18px;
  height: 18px;
}

.tenant-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 320px;
  overflow: auto;
  padding-right: 2px;
}

.tenant-item {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 14px 16px;
  border: 1px solid rgba(74, 142, 255, 0.16);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.04);
  color: #fff;
  text-align: left;
}

.tenant-item.selected {
  border-color: rgba(74, 142, 255, 0.7);
  background: rgba(74, 142, 255, 0.12);
}

.tenant-main {
  min-width: 0;
}

.tenant-name {
  font-size: 15px;
  font-weight: 600;
  line-height: 1.3;
}

.tenant-id {
  margin-top: 6px;
  color: rgba(217, 226, 255, 0.55);
  font-size: 12px;
  line-height: 1.3;
}

.tenant-meta {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
}

.tenant-radio {
  width: 18px;
  height: 18px;
  border: 2px solid rgba(217, 226, 255, 0.36);
  border-radius: 999px;
}

.tenant-radio.checked {
  border-color: #4a8eff;
  box-shadow: inset 0 0 0 4px #4a8eff;
}

.tenant-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 18px;
}

.tenant-secondary {
  padding: 11px 16px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.88);
}

.tenant-primary {
  padding: 11px 18px;
  border-radius: 12px;
  background: #4a8eff;
  color: #fff;
}

.tenant-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.tenant-fade-enter-active,
.tenant-fade-leave-active {
  transition: opacity 0.18s ease;
}

.tenant-fade-enter-from,
.tenant-fade-leave-to {
  opacity: 0;
}

@media (max-width: 767px) {
  .login-page {
    align-items: flex-start;
    overflow: auto;
  }

  .login-container {
    height: auto;
    min-height: calc(100vh - 32px);
    flex-direction: column;
    justify-content: center;
  }

  .intro-panel {
    width: 100%;
    padding: 24px 20px 0;
  }

  .brand-copy h1 {
    font-size: 44px;
  }

  .brand-copy p {
    font-size: 16px;
  }

  .feature-list {
    gap: 18px;
    padding-top: 8px;
  }

  .feature-item {
    align-items: flex-start;
    font-size: 16px;
  }

  .login-card-wrap {
    width: 100%;
  }

  .login-card {
    padding: 40px 28px;
  }
}
</style>

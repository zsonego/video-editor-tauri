<script setup>
import { onMounted, ref } from "vue";
import bgImage from "../assets/bg.png";
import logoImage from "../assets/logo1.png";

const emit = defineEmits(["login"]);

const account = ref("");
const password = ref("");
const passwordVisible = ref(false);

function togglePasswordVisible() {
  passwordVisible.value = !passwordVisible.value;
}

function handleBack() {
  if (window.history.length > 1) {
    window.history.back();
  }
}

function handleLogin() {
  // 原生页面没有登录请求，这里保留 Vue 表单入口，后续可接入接口。
  emit("login");
}

onMounted(() => {
  document.title = "灵剪·专业版 - 登录";
});
</script>

<template>
  <section
    class="login-page"
    :style="{ backgroundImage: `url(${bgImage})` }"
  >
    <main class="login-container" data-purpose="login-container">
      <div class="intro-panel">
        <div class="logo-wrap">
          <img :src="logoImage" alt="App Logo" class="app-logo" />
        </div>

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
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M10 19l-7-7m0 0l7-7m-7 7h18"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
              />
            </svg>
          </button>

          <div class="card-content">
            <div class="profile-icon">
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M20 21a8 8 0 0 0-16 0" />
                <circle cx="12" cy="7" r="4" />
              </svg>
            </div>

            <h2>账户登录</h2>

            <form class="login-form" @submit.prevent="handleLogin">
              <div class="form-row">
                <label for="login-account">账号：</label>
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
                <label for="login-password">密码：</label>
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
                    aria-hidden="true"
                  >
                    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" />
                    <circle cx="12" cy="12" r="3" />
                  </svg>
                  <svg v-else viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M3 3l18 18" />
                    <path d="M10.6 10.7A3 3 0 0 0 13.3 14" />
                    <path
                      d="M9.9 5.2A10.6 10.6 0 0 1 12 5c6.5 0 10 7 10 7a18 18 0 0 1-3.1 4.1"
                    />
                    <path
                      d="M6.7 6.7C3.7 8.7 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.2-1.5"
                    />
                  </svg>
                </button>
              </div>

              <div class="submit-wrap">
                <button class="login-button" type="submit">登录</button>
              </div>
            </form>

            <div class="card-footer">
              <p><br /></p>
            </div>
          </div>
        </div>
      </div>
    </main>
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
    "Segoe UI",
    sans-serif;
}

.login-container {
  width: 100%;
  max-width: 1280px;
  height: calc(100vh - 32px);
  max-height: 850px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 32px;
}

.intro-panel {
  width: 50%;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 32px;
  padding: 48px;
  color: #ffffff;
}

.logo-wrap {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  margin-bottom: 8px;
  border-radius: 16px;
}

.app-logo {
  width: 300px;
  height: auto;
  object-fit: contain;
  padding: 4px 0;
}

.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.brand-copy h1 {
  margin: 0;
  color: #ffffff;
  font-size: 60px;
  line-height: 1;
  font-weight: 700;
  letter-spacing: 0;
}

.brand-copy p {
  max-width: 448px;
  margin: 0;
  color: rgba(255, 255, 255, 0.8);
  font-size: 20px;
  line-height: 1.625;
}

.feature-list {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding-top: 24px;
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
  width: 450px;
  flex: 0 0 auto;
}

.login-card {
  position: relative;
  overflow: hidden;
  padding: 48px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 40px;
  background: rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.37);
  backdrop-filter: blur(25px);
  -webkit-backdrop-filter: blur(25px);
}

.back-button {
  position: absolute;
  top: 32px;
  left: 32px;
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
  transition: color 0.2s ease;
}

.back-button:hover {
  color: #ffffff;
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
  margin-bottom: 32px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 999px;
  background: rgba(26, 35, 126, 0.6);
  box-shadow:
    0 25px 50px -12px rgba(0, 0, 0, 0.25),
    0 8px 24px rgba(0, 0, 0, 0.2);
}

.profile-icon svg {
  width: 40px;
  height: 40px;
  color: rgba(255, 255, 255, 0.9);
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
}

.card-content h2 {
  margin: 0 0 40px;
  color: rgba(255, 255, 255, 0.7);
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
  gap: 32px;
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
  border-bottom: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 0;
  background: transparent;
  color: #ffffff;
  font: inherit;
  font-size: 14px;
  line-height: 1.4;
  outline: none;
  box-shadow: none;
  transition: border-color 0.2s ease;
}

.input-underline::placeholder {
  color: rgba(255, 255, 255, 0.4);
}

.input-underline:focus {
  border-bottom-color: #ffffff;
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
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  transition: color 0.2s ease;
}

.toggle-password:hover {
  color: #ffffff;
}

.toggle-password svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
  font-size: 20px;
}

.submit-wrap {
  padding-top: 24px;
}

.login-button {
  width: 100%;
  padding: 16px;
  border: 0;
  border-radius: 16px;
  background: #152b94;
  box-shadow: 0 0 20px rgba(21, 43, 148, 0.4);
  color: #ffffff;
  font: inherit;
  font-weight: 600;
  cursor: pointer;
  transition:
    transform 0.3s ease,
    background 0.3s ease;
}

.login-button:hover {
  background: #1e40af;
  transform: translateY(-2px);
}

.login-button:active {
  transform: scale(0.98);
}

.card-footer {
  margin-top: 40px;
  text-align: center;
}

.card-footer p {
  margin: 0;
  color: rgba(255, 255, 255, 0.6);
  font-size: 14px;
}

@media (max-width: 767px) {
  .login-page {
    align-items: flex-start;
    min-height: 100vh;
    overflow: auto;
    padding: 16px;
  }

  .login-container {
    height: auto;
    min-height: calc(100vh - 32px);
    flex-direction: column;
    justify-content: center;
  }

  .intro-panel {
    width: 100%;
    gap: 24px;
    padding: 24px;
  }

  .app-logo {
    width: min(300px, 80vw);
  }

  .brand-copy h1 {
    font-size: 48px;
  }

  .brand-copy p {
    font-size: 20px;
  }

  .login-card-wrap {
    width: 100%;
  }

  .login-card {
    padding: 40px;
  }
}

@media (max-width: 460px) {
  .intro-panel {
    padding: 16px;
  }

  .brand-copy h1 {
    font-size: 38px;
  }

  .brand-copy p,
  .feature-item {
    font-size: 16px;
  }

  .login-card {
    padding: 40px 28px;
  }
}
</style>

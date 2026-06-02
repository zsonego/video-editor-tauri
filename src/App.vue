<script setup>
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import SystemMessageHost from "./components/SystemMessageHost.vue";

const router = useRouter();
const appBootLoading = ref(true);

onMounted(() => {
  window.setTimeout(() => {
    appBootLoading.value = false;
  }, 450);
});

function handleLoginSuccess() {
  const redirect = typeof router.currentRoute.value.query.redirect === "string"
    ? router.currentRoute.value.query.redirect
    : "/home";

  router.replace(redirect);
}

function handleLogout() {
  localStorage.removeItem("token");
  localStorage.removeItem("userInfo");
  router.replace("/login");
}
</script>

<template>
  <Transition name="app-boot-fade">
    <div v-if="appBootLoading" class="app-boot-overlay">
      <div class="app-boot-panel">
        <div class="app-boot-ring"></div>
        <div class="app-boot-title">AICut loading...</div>
      </div>
    </div>
  </Transition>
  <RouterView v-slot="{ Component }">
    <component :is="Component" @login="handleLoginSuccess" @logout="handleLogout" />
  </RouterView>
  <SystemMessageHost />
</template>

<style>
* {
  box-sizing: border-box;
}

html,
body,
#app {
  width: 100%;
  min-height: 100%;
  margin: 0;
}

body {
  min-width: 320px;
  color: #ffffff;
  background: #030617;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

button,
input {
  font-family: inherit;
}

.app-boot-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    radial-gradient(circle at 50% 42%, rgba(74, 142, 255, 0.18), transparent 34%),
    #07122a;
  color: #d9e2ff;
}

.app-boot-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 18px;
}

.app-boot-ring {
  width: 52px;
  height: 52px;
  border-radius: 999px;
  border: 3px solid rgba(217, 226, 255, 0.16);
  border-top-color: #4a8eff;
  box-shadow: 0 0 22px rgba(74, 142, 255, 0.28);
  animation: appBootSpin 0.8s linear infinite;
}

.app-boot-title {
  font-size: 13px;
  font-weight: 800;
  letter-spacing: 0;
  color: rgba(217, 226, 255, 0.82);
}

.app-boot-fade-leave-active {
  transition: opacity 0.2s ease;
}

.app-boot-fade-leave-to {
  opacity: 0;
}

@keyframes appBootSpin {
  to {
    transform: rotate(360deg);
  }
}

::-webkit-scrollbar {
  width: 6px;
}

::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.1);
}

::-webkit-scrollbar-thumb {
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.2);
}

</style>

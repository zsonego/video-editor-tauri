<script setup>
import { useRouter } from "vue-router";
import SystemMessageHost from "./components/SystemMessageHost.vue";

const router = useRouter();

function handleLoginSuccess() {
  const redirect = typeof router.currentRoute.value.query.redirect === "string"
    ? router.currentRoute.value.query.redirect
    : "/home";

  router.replace(redirect);
}

function handleLogout() {
  localStorage.removeItem("loginState");
  localStorage.removeItem("userInfo");
  router.replace("/login");
}
</script>

<template>
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

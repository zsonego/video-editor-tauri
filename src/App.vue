<script setup>
import { ref } from "vue";
import LoginPage from "./views/LoginPage.vue";
import WorkspacePage from "./views/WorkspacePage.vue";

const currentPage = ref("login");
</script>

<template>
  <LoginPage v-if="currentPage === 'login'" @login="currentPage = 'workspace'" />
  <WorkspacePage v-else @logout="currentPage = 'login'" />
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

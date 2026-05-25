import { reactive } from "vue";

const state = reactive({
  items: [],
});

let seed = 0;

function remove(id) {
  const index = state.items.findIndex((item) => item.id === id);
  if (index !== -1) {
    const [item] = state.items.splice(index, 1);
    if (item.timer) {
      clearTimeout(item.timer);
    }
  }
}

function push(type, message, options = {}) {
  const id = ++seed;
  const duration = options.duration ?? 2600;
  const item = {
    id,
    type,
    message,
    timer: null,
  };

  state.items.push(item);

  if (duration > 0) {
    item.timer = window.setTimeout(() => remove(id), duration);
  }

  return id;
}

export const systemMessage = {
  state,
  success(message, options) {
    return push("success", message, options);
  },
  error(message, options) {
    return push("error", message, options);
  },
  info(message, options) {
    return push("info", message, options);
  },
  close(id) {
    remove(id);
  },
  clear() {
    [...state.items].forEach((item) => remove(item.id));
  },
};

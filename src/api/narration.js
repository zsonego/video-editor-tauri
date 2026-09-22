import { request } from "./request";

export function createNarrationTask(data) {
  return request("/api/narration/task/create", { data });
}

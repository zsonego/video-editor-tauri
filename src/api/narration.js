import { request } from "./request";

export function createNarrationTask(data) {
  return request("/api/narration/task/create", { data });
}

export function getNarrationScenePlanList(taskId) {
  const query = new URLSearchParams({
    taskId: String(taskId),
    recordType: "interview",
  });
  return request(`/api/narration/scene-plan/list?${query.toString()}`, {
    method: "GET",
  });
}

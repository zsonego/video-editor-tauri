import { request } from "./request";

export function getMyProjects(data) {
  return request("/api/project/my", { data });
}

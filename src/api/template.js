import { request } from "./request";
import { getStoredUploadBucket } from "../utils/uploadBuckets";

export function createTemplateDraft(renterId = "-1") {
  const query = new URLSearchParams({
    renterId: String(renterId || "-1"),
  });
  return request(`/aicut/template/draft?${query.toString()}`);
}

export function getTemplateCategories(data = {}) {
  return request("/api/template/category", { data });
}

export function getTemplates(data = {}) {
  return request("/api/template/list", { data });
}

export function queryMyTemplates(data = {}) {
  return request("/api/template/query", { data });
}

export function getTemplateDetail(data = {}) {
  return request("/api/template/detail", { data });
}

export function favoriteTemplate(data = {}) {
  return request("/api/template/favorite", { data });
}

export function getFavoriteTemplates(data = {}) {
  return request("/api/template/favorite/list", { data });
}

export async function downloadTemplateCover(templateId) {
  const bucket = getStoredUploadBucket("template-bucket");
  if (!bucket) {
    throw new Error("模板桶配置缺失，请重新登录");
  }

  return request("/aicut/file/download", {
    method: "GET",
    data: {
      bucket,
      path: `${templateId}/cover.png`,
      thumbnail: true,
    },
    responseType: "blob",
  });
}

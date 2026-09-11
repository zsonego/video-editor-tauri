import { request } from "./request";

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

export function getTemplateDetail(data = {}) {
  return request("/api/template/detail", { data });
}

export function getTemplateBosPresignedUrls(templateId) {
  return request("/api/aicut/file/bos/presigned-urls", {
    method: "GET",
    data: { templateId },
  });
}

export function favoriteTemplate(data = {}) {
  return request("/api/template/favorite", { data });
}

export function getFavoriteTemplates(data = {}) {
  return request("/api/template/favorite/list", { data });
}

export function downloadTemplateCover(templateId) {
  return request("/aicut/file/download", {
    method: "GET",
    data: {
      bucket: "template",
      path: `${templateId}/cover.png`,
      thumbnail: true,
    },
    responseType: "blob",
  });
}

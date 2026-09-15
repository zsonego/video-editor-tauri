import { invoke } from '@tauri-apps/api/core';
import { createTemplateDraft } from './template';
import { request } from './request';
import baiduSdkUrl from '../../baidubce-sdk.bundle.min.js?url';

const CHUNK_SIZE = 20 * 1024 * 1024;
const MAX_PART_RETRIES = 3;
const STATUS_POLL_INTERVAL_MS = 3000;
const STATUS_POLL_LIMIT = 200;

let sdkLoadPromise;

function responseData(response, action) {
  if (!response || Number(response.code) !== 0) {
    throw new Error(response?.msg || `${action}失败`);
  }
  return response.data || {};
}

function getRenterId() {
  try {
    const stored = JSON.parse(localStorage.getItem('userInfo') || 'null') || {};
    const profile = stored.user || stored.profile || stored.sysUser || stored;
    const renterId = stored.renterId ?? stored.tenantId ??
      profile.renterId ?? profile.tenantId;
    if (renterId === undefined || renterId === null || String(renterId).trim() === '') {
      throw new Error('登录信息中缺少 renterId');
    }
    return String(renterId);
  } catch (error) {
    if (error?.message === '登录信息中缺少 renterId') throw error;
    throw new Error('无法读取登录用户的 renterId');
  }
}

function makeUploadId(type) {
  return [type, Date.now().toString(36), Math.random().toString(36).slice(2, 10)].join('_');
}

function queryPath(path, params) {
  return `${path}?${new URLSearchParams(params).toString()}`;
}

function wait(milliseconds) {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

async function retryPart(action, type, partNumber) {
  let error;
  for (let attempt = 1; attempt <= MAX_PART_RETRIES; attempt += 1) {
    try {
      return await action();
    } catch (caught) {
      error = caught;
      if (attempt < MAX_PART_RETRIES) await wait(500 * attempt);
    }
  }
  throw new Error(`${type} 第 ${partNumber} 片上传失败：${error?.message || error}`);
}

async function loadBaiduSdk() {
  if (window.baidubce?.sdk?.BosClient) return window.baidubce.sdk;
  if (!sdkLoadPromise) {
    sdkLoadPromise = new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = baiduSdkUrl;
      script.onload = () => {
        const sdk = window.baidubce?.sdk;
        if (sdk?.BosClient) resolve(sdk);
        else reject(new Error('百度 BCE SDK 已加载，但未找到 BosClient'));
      };
      script.onerror = () => reject(new Error('百度 BCE SDK 加载失败'));
      document.head.appendChild(script);
    }).catch((error) => {
      sdkLoadPromise = null;
      throw error;
    });
  }
  return sdkLoadPromise;
}

async function uploadFileParts({
  localTemplateKey,
  type,
  fileSize,
  totalChunks,
  scene,
  uploadId,
  directUpload,
  onProgress,
}) {
  const direct = type === 'assets';
  let client;
  if (direct) {
    if (!directUpload?.uploadId || !directUpload?.bucket || !directUpload?.objectKey ||
        !directUpload?.endpoint || !directUpload?.accessKeyId ||
        !directUpload?.secretAccessKey || !directUpload?.sessionToken) {
      throw new Error('素材包初始化成功，但百度直传凭证不完整');
    }
    const sdk = await loadBaiduSdk();
    client = new sdk.BosClient({
      endpoint: directUpload.endpoint,
      credentials: {
        ak: directUpload.accessKeyId,
        sk: directUpload.secretAccessKey,
      },
      sessionToken: directUpload.sessionToken,
    });
  }

  let nextIndex = 0;
  let completed = 0;
  let stopped = false;
  const workers = Array.from({ length: Math.min(direct ? 4 : 1, totalChunks) }, async () => {
    while (!stopped && nextIndex < totalChunks) {
      const index = nextIndex;
      nextIndex += 1;
      try {
        await retryPart(async () => {
          const bytes = await invoke('read_custom_template_upload_chunk', {
            localTemplateKey,
            fileType: type,
            chunkIndex: index,
            chunkSize: CHUNK_SIZE,
          });
          const blob = new Blob([bytes], { type: 'application/octet-stream' });
          if (!blob.size || blob.size > CHUNK_SIZE ||
              blob.size !== Math.min(CHUNK_SIZE, fileSize - index * CHUNK_SIZE)) {
            throw new Error('读取到的上传分片大小不正确');
          }
          if (direct) {
            await client.uploadPartFromBlob(
              directUpload.bucket,
              directUpload.objectKey,
              directUpload.uploadId,
              index + 1,
              blob.size,
              blob,
            );
          } else {
            const form = new FormData();
            form.append('uploadId', uploadId);
            form.append('chunkIndex', String(index));
            form.append('totalChunks', String(totalChunks));
            form.append('scene', scene);
            form.append('file', blob, `${type}_${index}`);
            responseData(
              await request('/aicut/file/chunk', {
                data: form,
                headers: { 'Content-Type': 'multipart/form-data' },
                timeout: 120000,
              }),
              `${type} 分片上传`,
            );
          }
        }, type, index + 1);
      } catch (error) {
        stopped = true;
        throw error;
      }
      completed += 1;
      onProgress(completed / totalChunks);
    }
  });
  const results = await Promise.allSettled(workers);
  const failed = results.find((result) => result.status === 'rejected');
  if (failed) throw failed.reason;
}

async function waitForReady(uploadId, scene, type, onStatus) {
  for (let poll = 0; poll < STATUS_POLL_LIMIT; poll += 1) {
    const data = responseData(
      await request(queryPath('/aicut/file/chunk/status', { uploadId, scene }), {
        method: 'GET',
        timeout: 30000,
      }),
      `${type} 状态查询`,
    );
    const status = String(data.status || '').toUpperCase();
    onStatus(status);
    if (status === 'READY') return;
    if (status === 'FAILED' || status === 'ABORTED') {
      throw new Error(`${type} 上传${status === 'FAILED' ? '校验失败' : '已中止'}：${data.error || '未知原因'}`);
    }
    if (!['UPLOADING', 'VALIDATING', 'MERGING'].includes(status)) {
      throw new Error(`${type} 返回未知上传状态：${status || '空'}`);
    }
    await wait(STATUS_POLL_INTERVAL_MS);
  }
  throw new Error(`${type} 上传状态等待超时，请稍后重试`);
}

async function uploadOneFile({
  localTemplateKey,
  templateId,
  scene,
  type,
  fileName,
  fileSize,
  onProgress,
  onStatus,
}) {
  if (!Number.isSafeInteger(fileSize) || fileSize <= 0) {
    throw new Error(`${fileName} 为空或文件大小无效`);
  }
  const totalChunks = Math.ceil(fileSize / CHUNK_SIZE);
  const initialUploadId = makeUploadId(type);
  const init = responseData(
    await request(queryPath('/aicut/file/chunk/init', {
      uploadId: initialUploadId,
      type,
      fileName,
      fileSize: String(fileSize),
      totalChunks: String(totalChunks),
      chunkSize: String(CHUNK_SIZE),
      templateId: String(templateId),
      scene,
    }), { timeout: 30000 }),
    `${fileName} 初始化`,
  );
  const uploadId = String(init.uploadId || '').trim();
  if (!uploadId) throw new Error(`${fileName} 初始化成功，但没有返回 uploadId`);
  await uploadFileParts({
    localTemplateKey,
    type,
    fileSize,
    totalChunks,
    scene,
    uploadId,
    directUpload: init.directUpload,
    onProgress,
  });

  const form = new FormData();
  form.append('type', type);
  form.append('uploadId', uploadId);
  form.append('fileName', fileName);
  form.append('totalChunks', String(totalChunks));
  form.append('templateId', String(templateId));
  form.append('scene', scene);
  responseData(
    await request('/aicut/file/chunk/merge', {
      data: form,
      headers: { 'Content-Type': 'multipart/form-data' },
      timeout: 120000,
    }),
    `${fileName} 合并`,
  );
  await waitForReady(uploadId, scene, type, onStatus);
}

export async function submitLocalTemplate(template, onUpdate) {
  const localTemplateKey = String(template?.templateId || '').trim();
  if (!localTemplateKey) throw new Error('本地模板目录名缺失');
  const renterId = getRenterId();
  onUpdate({ stage: '正在整理模板文件并压缩素材包...', progress: 1 });
  const files = await invoke('prepare_custom_template_upload', { localTemplateKey });

  let backendTemplateId = files.backendTemplateId;
  const scene = files.finalized ? 'edit' : 'add';
  if (!backendTemplateId) {
    onUpdate({ stage: '正在创建模板草稿...', progress: 5 });
    const draft = responseData(await createTemplateDraft(renterId), '创建模板草稿');
    backendTemplateId = draft.templateId;
    if (backendTemplateId === undefined || backendTemplateId === null || backendTemplateId === '') {
      throw new Error('创建模板草稿成功，但未返回 templateId');
    }
    backendTemplateId = String(backendTemplateId);
    await invoke('save_custom_template_upload_state', {
      localTemplateKey,
      backendTemplateId,
      finalized: false,
    });
  }

  const tasks = [
    { type: 'xml', fileName: 'template.xml', fileSize: files.xmlSize, start: 10, end: 30 },
    { type: 'cover', fileName: 'cover.png', fileSize: files.coverSize, start: 30, end: 45 },
    { type: 'assets', fileName: 'assets.zip', fileSize: files.assetsSize, start: 45, end: 95 },
  ];
  for (const task of tasks) {
    onUpdate({
      stage: `正在上传 ${task.fileName}...`,
      progress: task.start,
    });
    await uploadOneFile({
      localTemplateKey,
      templateId: backendTemplateId,
      scene,
      ...task,
      onProgress: (fraction) => onUpdate({
        stage: `正在上传 ${task.fileName}...`,
        progress: Math.round(task.start + (task.end - task.start - 2) * fraction),
      }),
      onStatus: (status) => onUpdate({
        stage: `${task.fileName}：${status === 'VALIDATING' ? '正在校验' : status === 'READY' ? '上传完成' : status}`,
        progress: task.end,
      }),
    });
  }

  onUpdate({ stage: '正在提交正式模板...', progress: 97 });
  const result = responseData(
    await request('/aicut/template', {
      data: {
        id: Number(backendTemplateId),
        renterId,
        name: String(template.name || '').trim(),
        version: '1.0',
        category: '',
        score: 0,
        coverPic: 'cover.png',
        xmlPath: 'template.xml',
        assetsPath: 'assets.zip',
        tags: [],
      },
      timeout: 30000,
    }),
    '提交正式模板',
  );
  if (result.templateId !== undefined && String(result.templateId) !== String(backendTemplateId)) {
    throw new Error('正式模板接口返回了不同的 templateId，请联系后端核实');
  }
  await invoke('save_custom_template_upload_state', {
    localTemplateKey,
    backendTemplateId: String(backendTemplateId),
    finalized: true,
  });
  onUpdate({ stage: '模板上传完成', progress: 100 });
  return backendTemplateId;
}

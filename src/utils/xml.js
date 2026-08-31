const ID_ALPHABET = 'abcdefghijklmnopqrstuvwxyz0123456789';

export function generateId(length = 21) {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return Array.from(
    bytes,
    (value) => ID_ALPHABET[value % ID_ALPHABET.length],
  ).join('');
}

const attr = (value) =>
  String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');

const text = (value) =>
  String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');

const n = (value, fallback = 0) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
};

const opacity = (value) => Math.min(1, Math.max(0, n(value, 1)));
const progress = (value) => Math.min(1, Math.max(0, n(value, 0)));
const percent = (value, fallback = 0) =>
  Math.min(100, Math.max(0, n(value, fallback)));
const bool = (value) => value === true || value === 'true' || value === '1';

const directChild = (node, tag) =>
  Array.from(node?.children ?? []).find((child) => child.tagName === tag) ??
  null;

const directChildren = (node, tag) =>
  Array.from(node?.children ?? []).filter((child) => child.tagName === tag);

const childText = (node, tag, fallback = '') =>
  directChild(node, tag)?.textContent?.trim() ?? fallback;

const filepathName = (path = '') => path.split(/[\\/]/).pop() || '';

export function assetPath(filename) {
  return filename ? `template/assets/${filename}` : '';
}

export function buildXml(model) {
  const lines = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE xmeml>',
    '<xmeml version="5">',
    `    <template id="${attr(model.id)}" name="${attr(model.name)}" version="1.0" timeunit="millisecond">`,
    '        <video>',
    `            <duration>${n(model.duration)}</duration>`,
    `            <resolution>${text(model.resolution)}</resolution>`,
    `            <style>${text(model.videoStyle || 'cinematic')}</style>`,
    `            <progress>${progress(model.progress)}</progress>`,
    `            <demo-path>${text(model.demoPath)}</demo-path>`,
    '        </video>',
    '        <tracks>',
  ];

  if (model.tracks.background) {
    lines.push(
      '            <track id="bg" z-index="0">',
      `                <filepath>${text(model.tracks.background)}</filepath>`,
      '            </track>',
    );
  }

  lines.push('            <track id="clips" z-index="1" />');
  if (model.tracks.overlay) {
    lines.push(
      '            <track id="overlay" z-index="2">',
      `                <filepath>${text(model.tracks.overlay)}</filepath>`,
      '            </track>',
    );
  }

  if (model.tracks.audioBackground) {
    lines.push(
      '            <track id="audio-bg" z-index="3">',
      `                <filepath>${text(model.tracks.audioBackground)}</filepath>`,
      '            </track>',
    );
  }

  if (model.tracks.recording) {
    lines.push(
      '            <track id="recording" z-index="4">',
      `                <filepath>${text(model.tracks.recording)}</filepath>`,
      '            </track>',
    );
  }

  lines.push('        </tracks>');

  model.mediaGroups.forEach((group) => {
    lines.push(
      `        <media-asset id="${attr(group.id)}" name="${attr(group.name)}">`,
      `            <type>${group.mediaType === 'audio' ? 'audio' : 'video'}</type>`,
      '            <constraints>',
      `                <minDuration>${n(group.minDuration)}</minDuration>`,
      `                <maxDuration>${n(group.maxDuration)}</maxDuration>`,
      '            </constraints>',
      '            <default-asset>',
    );
    group.assets.forEach((assetItem) => {
      lines.push(
        `                <asset id="${attr(assetItem.id)}" filepath="${attr(assetItem.filepath)}" />`,
      );
    });
    lines.push('            </default-asset>', '        </media-asset>');
  });

  lines.push(
    `        <clips id="${attr(model.clipsId)}" target-track="clips">`,
  );

  model.clips.forEach((clip) => {
    lines.push(
      `            <clip id="${attr(clip.id)}" name="${attr(clip.name)}" material-type="${attr(clip.materialType === 'fixed' ? 'fixed' : 'variable')}">`,
      `                <starttime>${n(clip.starttime)}</starttime>`,
      `                <duration>${n(clip.duration)}</duration>`,
    );

    if (clip.topVideo) {
      lines.push(`                <top-video>${text(clip.topVideo)}</top-video>`);
    }

    clip.areas.forEach((area) => {
      lines.push(
        `                <area id="${attr(area.id)}" asset-id="${attr(area.assetId)}" index="${attr(Math.max(0, Math.round(n(area.index, 1))))}" opacity="${attr(opacity(area.opacity))}">`,
        '                    <source>',
        `                        <duration>${n(clip.duration)}</duration>`,
        '                    </source>',
        '                    <transform>',
        `                        <mirror>${text(area.mirror)}</mirror>`,
        `                        <speed>${n(area.speed, 1)}</speed>`,
        `                        <rotate>${n(area.rotate)}</rotate>`,
        '                    </transform>',
        `                    <beauty lut-style="${attr(area.beauty?.lutStyle || 'none')}" lut-intensity="${attr(percent(area.beauty?.lutIntensity, 100))}" skin-tone="${attr(area.beauty?.skinTone || 'off')}" skin-intensity="${attr(percent(area.beauty?.skinIntensity, 60))}" smoothing="${attr(percent(area.beauty?.smoothing))}" whitening="${attr(percent(area.beauty?.whitening))}" stabilization="${attr(Boolean(area.beauty?.stabilization))}" one-click="${attr(Boolean(area.beauty?.oneClickBeauty))}" />`,
        '                    <destination>',
        `                        <position x="${attr(n(area.x))}" y="${attr(n(area.y))}" />`,
        `                        <width>${n(area.width, 1920)}</width>`,
        `                        <height>${n(area.height, 1080)}</height>`,
        '                    </destination>',
        '                </area>',
      );
    });

    clip.subtitles.forEach((subtitle) => {
      const timeAttribute =
        subtitle.timeMode === 'absolute' ? 'absoluteStartTime' : 'startTime';
      lines.push(
        `                <subtitle id="${attr(subtitle.id)}" ${timeAttribute}="${attr(n(subtitle.time))}" duration="${attr(n(subtitle.duration))}">`,
        `                    <minlen>${n(subtitle.minlen)}</minlen>`,
        `                    <maxlen>${n(subtitle.maxlen)}</maxlen>`,
        `                    <default>${text(subtitle.defaultText)}</default>`,
        `                    <font family="${attr(subtitle.fontFamily)}" size="${attr(n(subtitle.fontSize))}" color="${attr(subtitle.color)}" />`,
        `                    <position x="${attr(n(subtitle.x))}" y="${attr(n(subtitle.y))}" />`,
        '                </subtitle>',
      );
    });

    if (clip.transition.enabled) {
      lines.push(
        '                <filter>',
        `                    <effect>${text(clip.transition.effect)}</effect>`,
        `                    <duration>${n(clip.transition.duration)}</duration>`,
        '                </filter>',
      );
    }

    lines.push('            </clip>');
  });

  lines.push('        </clips>', '    </template>', '</xmeml>', '');
  return lines.join('\n');
}

export function parseXml(xmlText) {
  const documentNode = new DOMParser().parseFromString(
    xmlText,
    'application/xml',
  );
  const parserError = documentNode.querySelector('parsererror');
  if (parserError) {
    throw new Error('XML 格式无法解析，请检查标签是否完整。');
  }

  const template = documentNode.querySelector('template');
  if (!template) throw new Error('未找到 template 节点。');

  const video = directChild(template, 'video');
  const tracksNode = directChild(template, 'tracks');
  const trackNodes = directChildren(tracksNode, 'track');
  const findTrack = (id) =>
    trackNodes.find((track) => track.getAttribute('id') === id);
  const readTrackPath = (id) => childText(findTrack(id), 'filepath');

  const assetIdMap = new Map();
  const mediaGroups = directChildren(template, 'media-asset').map(
    (groupNode) => {
      const groupName = groupNode.getAttribute('name') || '未命名目录';
      const mediaType =
        childText(groupNode, 'type', 'video') === 'audio' ||
        groupName === '默认音频' ||
        groupName === '录音音频'
          ? 'audio'
          : 'video';
      const constraints = directChild(groupNode, 'constraints');
      const assetsNode = directChild(groupNode, 'default-asset');
      const assets = directChildren(assetsNode, 'asset').map((assetNode) => {
        const oldId = assetNode.getAttribute('id') || generateId();
        const newId = generateId();
        assetIdMap.set(oldId, newId);
        const filepath = assetNode.getAttribute('filepath') || '';
        return {
          id: newId,
          name: filepathName(filepath),
          filepath,
          sourcePath: '',
          mediaType,
          thumbnailUrl: '',
          thumbnailStatus: 'unavailable',
          durationMs: 0,
          width: 0,
          height: 0,
          disposed: false,
        };
      });
      return {
        id: generateId(),
        name: groupName,
        mediaType,
        minDuration: n(childText(constraints, 'minDuration'), 3000),
        maxDuration: n(childText(constraints, 'maxDuration'), 10000),
        expanded: true,
        assets,
      };
    },
  );

  const clipsNode = directChild(template, 'clips');
  const clips = directChildren(clipsNode, 'clip').map((clipNode, clipIndex) => {
    const clipDuration = n(childText(clipNode, 'duration'), 3000);
    const areas = directChildren(clipNode, 'area').map(
      (areaNode, areaIndex) => {
        const transform = directChild(areaNode, 'transform');
        const beauty = directChild(areaNode, 'beauty');
        const destination = directChild(areaNode, 'destination');
        const position = directChild(destination, 'position');
        const oldAssetId = areaNode.getAttribute('asset-id') || '';
        return {
          id: generateId(),
          assetId: assetIdMap.get(oldAssetId) || '',
          index: Math.max(
            0,
            Math.round(n(areaNode.getAttribute('index'), areaIndex + 1)),
          ),
          opacity: opacity(areaNode.getAttribute('opacity')),
          mirror: childText(transform, 'mirror', 'none') || 'none',
          speed: n(childText(transform, 'speed'), 1),
          rotate: n(childText(transform, 'rotate'), 0),
          beauty: {
            lutStyle: beauty?.getAttribute('lut-style') || 'none',
            lutIntensity: percent(
              beauty?.getAttribute('lut-intensity'),
              100,
            ),
            skinTone: beauty?.getAttribute('skin-tone') || 'off',
            skinIntensity: percent(
              beauty?.getAttribute('skin-intensity'),
              60,
            ),
            smoothing: percent(beauty?.getAttribute('smoothing')),
            whitening: percent(beauty?.getAttribute('whitening')),
            stabilization: bool(beauty?.getAttribute('stabilization')),
            oneClickBeauty: bool(beauty?.getAttribute('one-click')),
          },
          x: n(position?.getAttribute('x')),
          y: n(position?.getAttribute('y')),
          width: n(childText(destination, 'width'), 1920),
          height: n(childText(destination, 'height'), 1080),
        };
      },
    );

    const subtitles = directChildren(clipNode, 'subtitle').map(
      (subtitleNode) => {
        const font = directChild(subtitleNode, 'font');
        const position = directChild(subtitleNode, 'position');
        const isAbsolute = subtitleNode.hasAttribute('absoluteStartTime');
        return {
          id: generateId(),
          timeMode: isAbsolute ? 'absolute' : 'relative',
          time: n(
            subtitleNode.getAttribute(
              isAbsolute ? 'absoluteStartTime' : 'startTime',
            ),
          ),
          duration: n(subtitleNode.getAttribute('duration'), 5000),
          minlen: n(childText(subtitleNode, 'minlen'), 2),
          maxlen: n(childText(subtitleNode, 'maxlen'), 20),
          defaultText: childText(subtitleNode, 'default'),
          fontFamily: font?.getAttribute('family') || 'Songti SC',
          fontSize: n(font?.getAttribute('size'), 60),
          color: font?.getAttribute('color') || '#ffffff',
          x: n(position?.getAttribute('x'), 960),
          y: n(position?.getAttribute('y'), 900),
          expanded: true,
        };
      },
    );

    const filter = directChild(clipNode, 'filter');
    return {
      id: generateId(),
      name: clipNode.getAttribute('name') || `片段 ${clipIndex + 1}`,
      materialType:
        clipNode.getAttribute('material-type') === 'fixed'
          ? 'fixed'
          : 'variable',
      starttime: n(childText(clipNode, 'starttime')),
      duration: clipDuration,
      topVideo: childText(clipNode, 'top-video'),
      topVideoSourcePath: '',
      areas,
      subtitles,
      transition: {
        enabled: Boolean(filter),
        effect: childText(filter, 'effect', 'Fade') || 'Fade',
        duration: n(childText(filter, 'duration'), 500),
      },
    };
  }).sort((left, right) => left.starttime - right.starttime);

  return {
    id: generateId(),
    clipsId: generateId(),
    name: template.getAttribute('name') || '未命名模板',
    duration: n(childText(video, 'duration')),
    resolution: childText(video, 'resolution', '1920*1080') || '1920*1080',
    videoStyle: childText(video, 'style', 'cinematic') || 'cinematic',
    progress: progress(childText(video, 'progress')),
    demoPath: childText(video, 'demo-path'),
    tracks: {
      background: readTrackPath('bg'),
      overlay: readTrackPath('overlay'),
      audioBackground: readTrackPath('audio-bg'),
      recording: readTrackPath('recording'),
    },
    mediaGroups,
    clips,
  };
}

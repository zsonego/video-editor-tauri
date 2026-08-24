<script setup>
import { computed } from 'vue';
import {
  ArrowLeft,
  ArrowRight,
  BadgeDollarSign,
  BookOpen,
  Captions,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  CircleHelp,
  CircleUserRound,
  CloudDownload,
  Compass,
  Download,
  FastForward,
  FileText,
  History,
  KeyRound,
  LoaderCircle,
  LogOut,
  MonitorPlay,
  Minus,
  Pause,
  Play,
  Plus,
  RefreshCcw,
  RotateCcw,
  RotateCw,
  Search,
  SlidersVertical,
  Sparkles,
  Square,
  SquareCheckBig,
  SquarePlus,
  Star,
  Trash2,
  TriangleAlert,
  User,
  Volume2,
  VolumeX,
  X,
} from '@lucide/vue';

const props = defineProps({
  name: {
    type: String,
    required: true,
  },
  size: {
    type: [Number, String],
    default: 24,
  },
  strokeWidth: {
    type: [Number, String],
    default: 2,
  },
  filled: {
    type: Boolean,
    default: false,
  },
});

const iconMap = {
  account_circle: CircleUserRound,
  add_box: SquarePlus,
  arrow_back: ArrowLeft,
  arrow_forward: ArrowRight,
  check_box: SquareCheckBig,
  check_box_outline_blank: Square,
  chevron_left: ChevronLeft,
  chevron_right: ChevronRight,
  close: X,
  delete: Trash2,
  download: Download,
  downloading: CloudDownload,
  expand_more: ChevronDown,
  explore: Compass,
  fast_forward: FastForward,
  description: FileText,
  forward_10: RotateCw,
  help_center: CircleHelp,
  history: History,
  keyboard_arrow_down: ChevronDown,
  keyboard_arrow_up: ChevronUp,
  lock_reset: KeyRound,
  logout: LogOut,
  menu_book: BookOpen,
  remove: Minus,
  pause: Pause,
  payments: BadgeDollarSign,
  person: User,
  play_arrow: Play,
  add: Plus,
  progress_activity: LoaderCircle,
  quiz: CircleHelp,
  replay_10: RotateCcw,
  search: Search,
  smart_display: MonitorPlay,
  audio_settings: SlidersVertical,
  subtitles: Captions,
  star: Star,
  stars: Sparkles,
  sync_problem: RefreshCcw,
  volume_off: VolumeX,
  volume_up: Volume2,
  warning: TriangleAlert,
};

const iconComponent = computed(() => iconMap[props.name] || CircleHelp);
</script>

<template>
  <component
    :is="iconComponent"
    :size="size"
    :stroke-width="strokeWidth"
    :fill="filled ? 'currentColor' : 'none'"
    aria-hidden="true"
    focusable="false"
  />
</template>

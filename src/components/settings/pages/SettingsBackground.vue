<template>
  <MenuPage>
    <!-- ========== 场景管理 ========== -->
    <MenuItem :title="$t('settings.background.scene.title')">
      <template #header>
        <PictureInPicture :size="20" />
      </template>

      <!-- 当前场景信息 + 操作按钮 -->
      <div class="mb-4 flex items-center gap-3">
        <div class="text-brand font-bold">
          {{ $t("settings.background.scene.current") }}{{ currentSceneDisplay }}
        </div>
        <div class="ml-auto flex gap-3">
          <button
            class="bg-brand/80 border-brand hover:bg-brand rounded-full border px-5 py-1.5 text-sm
              font-bold text-white shadow-lg shadow-indigo-500/20 transition-all"
            @click="handleCreateScene"
          >
            {{ $t("settings.background.scene.create") }}
          </button>
          <button
            class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold
              text-white/80 shadow-lg transition-all hover:bg-white/20 disabled:cursor-not-allowed
              disabled:opacity-40"
            :disabled="isBackgroundCategoryReadOnly"
            @click="triggerUpload"
          >
            {{ $t("settings.background.scene.upload") }}
          </button>
          <button
            v-if="!isAndroid()"
            class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold
              text-white/80 shadow-lg transition-all hover:bg-white/20"
            @click="handleOpenFolder"
          >
            {{ $t("settings.background.scene.openFolder") }}
          </button>
          <button
            class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold
              text-white/80 shadow-lg transition-all hover:bg-white/20"
            @click="openSortModal"
          >
            {{ $t("settings.background.sort.button") }}
          </button>
          <button
            class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold
              text-white/80 shadow-lg transition-all hover:bg-white/20"
            @click="handleRefreshScenes"
            :title="$t('settings.background.scene.refreshTip')"
          >
            {{ $t("settings.background.scene.refresh") }}
          </button>
          <button
            class="rounded-full border border-red-500/30 bg-red-500/20 px-4 py-1.5 text-sm font-bold
              text-red-300 shadow-lg transition-all hover:bg-red-500/30"
            :disabled="!currentScene"
            @click="handleDeleteScene"
          >
            {{ $t("settings.background.scene.delete") }}
          </button>
        </div>
      </div>

      <!-- 背景分类管理（子文件夹 = 子分类）：选项卡 + 新建 + 删除 -->
      <div class="mb-4 flex flex-wrap items-center gap-2">
        <button
          class="rounded-full border px-3 py-1 text-xs font-semibold transition-all"
          :class="
            currentBackgroundCategory === '全部'
              ? 'bg-brand/80 border-brand text-white'
              : 'border-white/20 bg-white/10 text-white/70 hover:bg-white/20'
          "
          @click="currentBackgroundCategory = '全部'"
        >
          {{ $t("settings.background.scene.categoryAll") }}
        </button>
        <button
          v-for="cat in backgroundCategories"
          :key="cat"
          class="rounded-full border px-3 py-1 text-xs font-semibold transition-all"
          :class="
            currentBackgroundCategory === cat
              ? 'bg-brand/80 border-brand text-white'
              : 'border-white/20 bg-white/10 text-white/70 hover:bg-white/20'
          "
          @click="currentBackgroundCategory = cat"
        >
          {{ cat }}
        </button>

        <!-- 新建分类 -->
        <div class="flex items-center gap-1">
          <input
            v-model="newCategoryName"
            :placeholder="$t('settings.background.scene.categoryNamePlaceholder')"
            class="w-28 rounded-lg border border-white/15 bg-black/30 px-2 py-1 text-xs text-white
              focus:border-indigo-400 focus:outline-none"
            @keyup.enter="handleCreateCategory"
          />
          <button
            class="rounded-full border border-indigo-400 bg-indigo-500/80 px-2.5 py-1 text-xs
              font-semibold text-white hover:bg-indigo-500"
            @click="handleCreateCategory"
          >
            {{ $t("settings.background.scene.categoryAdd") }}
          </button>
        </div>

        <!-- 删除当前选中的分类（非"全部"时显示） -->
        <button
          v-if="currentBackgroundCategory !== '全部' && !isBackgroundCategoryReadOnly"
          class="rounded-full border border-red-400/40 bg-red-500/20 px-2.5 py-1 text-xs
            font-semibold text-red-300 hover:bg-red-500/30"
          @click="handleDeleteCategoryFlow"
        >
          {{ $t("settings.background.scene.categoryDelete") }}
        </button>
      </div>

      <!-- 场景卡片网格 -->
      <div class="grid w-full grid-cols-1 gap-5 pb-5 sm:grid-cols-2 xl:grid-cols-3">
        <div
          v-for="scene in paginatedScenes"
          :key="scene.id"
          :class="[
            `group relative flex cursor-pointer flex-col overflow-hidden rounded-xl border
            border-white/12.5 bg-white/10
            shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]
            backdrop-blur-[20px] backdrop-saturate-180 transition-all duration-300
            hover:-translate-y-1 hover:scale-[1.01] hover:bg-white/15
            hover:shadow-[0_12px_40px_rgba(0,0,0,0.15),inset_0_2px_2px_rgba(255,255,255,0.15)]
            hover:backdrop-blur-[25px] hover:backdrop-saturate-200`,
            isSceneSelected(scene.id)
              ? `border-2! border-sky-400!
                shadow-[0_0_12px_rgba(56,189,248,0.5),0_0_3px_rgba(56,189,248,0.8),inset_0_0_8px_rgba(56,189,248,0.15)]`
              : '',
          ]"
          @click="handleSceneClick(scene)"
          @contextmenu.prevent="openSceneContextMenu(scene, $event)"
        >
          <!-- 编辑按钮（右上角扳手）—— 插件场景只读，不提供编辑 -->
          <button
            v-if="!scene.source || scene.source === 'game'"
            class="absolute top-2 right-2 z-10 rounded-lg bg-black/50 p-1.5 text-white/60 opacity-0
              transition-all group-hover:opacity-100 hover:bg-black/70 hover:text-white"
            @click.stop="handleWrenchClick(scene)"
            :title="$t('settings.background.scene.edit')"
          >
            <Wrench :size="16" />
          </button>
          <!-- 收藏置顶按钮（左上角星星） -->
          <button
            class="absolute top-2 left-2 z-10 rounded-lg bg-black/50 p-1.5 transition-all"
            @click.stop="handleToggleFavorite(scene)"
            :title="
              isFavored(scene.id)
                ? $t('settings.background.scene.unfav')
                : $t('settings.background.scene.fav')
            "
          >
            <Star
              :size="16"
              :class="
                isFavored(scene.id)
                  ? 'fill-amber-400 text-amber-400'
                  : 'text-white/60 hover:text-white'
              "
            />
          </button>
          <!-- 插件来源标签（右上角） -->
          <PluginTag
            v-if="scene.source && scene.source !== 'game'"
            :source="scene.source"
            class="absolute top-2 right-2 z-10"
          />

          <!-- 背景预览 -->
          <div
            class="relative flex-1 overflow-hidden after:pointer-events-none after:absolute
              after:inset-0 after:bg-linear-to-b after:from-transparent after:to-black/30"
          >
            <img
              v-if="scene.background"
              :src="convertFileSrc(scene.background)"
              :alt="scene.scene_name"
              class="aspect-video h-full w-full object-cover transition-transform duration-300
                group-hover:scale-[1.03]"
            />
            <div
              v-else
              class="flex aspect-video h-full w-full items-center justify-center bg-black/40
                text-white/20"
            >
              <Image :size="48" />
            </div>
          </div>

          <!-- 信息栏 -->
          <div
            class="relative z-2 flex flex-col gap-1 border-t border-white/20 bg-white/15 px-4 py-3
              backdrop-blur-[10px]"
          >
            <span class="truncate font-medium text-white/90 drop-shadow-md">
              {{ scene.scene_name }}
            </span>
            <span v-if="scene.scene_description" class="line-clamp-2 text-xs text-white/50">{{
              scene.scene_description
            }}</span>
            <span v-else class="text-xs text-yellow-400/60 italic">{{
              $t("settings.background.scene.noDescription")
            }}</span>
          </div>
        </div>
      </div>

      <!-- 场景右键菜单：移动到子分类 -->
      <div
        v-if="sceneMenu.visible"
        class="fixed inset-0 z-[9998]"
        @click="closeSceneContextMenu"
        @contextmenu.prevent="closeSceneContextMenu"
      ></div>
      <div
        v-if="sceneMenu.visible && sceneMenu.scene"
        class="fixed z-[9999] min-w-44 rounded-xl border border-white/15 bg-slate-900/95 p-1.5
          shadow-2xl backdrop-blur-xl"
        :style="sceneMenuStyle"
        @click.stop
      >
        <div class="px-2.5 py-1.5 text-xs font-semibold text-white/40">
          {{ $t("settings.background.scene.moveToTitle") }}
        </div>
        <button
          class="block w-full rounded-lg px-2.5 py-1.5 text-left text-sm text-white/80
            transition-colors hover:bg-white/10"
          @click="handleMoveScene(sceneMenu.scene, '根目录')"
        >
          {{ $t("settings.background.scene.moveToRoot") }}
        </button>
        <button
          v-for="cat in writableBackgroundCategories"
          :key="'move-' + cat"
          class="block w-full rounded-lg px-2.5 py-1.5 text-left text-sm text-white/80
            transition-colors hover:bg-white/10"
          @click="handleMoveScene(sceneMenu.scene, cat)"
        >
          {{ cat }}
        </button>
        <div
          v-if="writableBackgroundCategories.length === 0"
          class="px-2.5 py-1.5 text-xs text-white/30"
        >
          {{ $t("settings.background.scene.moveNoCategory") }}
        </div>
      </div>

      <!-- 分页控件 -->
      <div v-if="totalPages > 1" class="flex items-center justify-center gap-2 pb-2">
        <button
          :disabled="currentPage <= 1"
          @click="currentPage = 1"
          class="px-2 py-1 text-xs text-white/50 transition-colors hover:text-white
            disabled:cursor-not-allowed disabled:opacity-30"
        >
          {{ $t("settings.background.pagination.first") }}
        </button>
        <button
          :disabled="currentPage <= 1"
          @click="currentPage = currentPage - 1"
          class="px-3 py-1 text-sm text-white/50 transition-colors hover:text-white
            disabled:cursor-not-allowed disabled:opacity-30"
        >
          {{ $t("settings.shared.prevPage") }}
        </button>
        <span class="px-3 text-xs text-white/60">
          {{ $t("settings.shared.pageOf", { current: currentPage, total: totalPages }) }}
        </span>
        <button
          :disabled="currentPage >= totalPages"
          @click="currentPage = currentPage + 1"
          class="px-3 py-1 text-sm text-white/50 transition-colors hover:text-white
            disabled:cursor-not-allowed disabled:opacity-30"
        >
          {{ $t("settings.shared.nextPage") }}
        </button>
        <button
          :disabled="currentPage >= totalPages"
          @click="currentPage = totalPages"
          class="px-2 py-1 text-xs text-white/50 transition-colors hover:text-white
            disabled:cursor-not-allowed disabled:opacity-30"
        >
          {{ $t("settings.background.pagination.last") }}
        </button>
      </div>

      <!-- 隐藏的文件上传 input -->
      <input
        type="file"
        ref="uploadInput"
        @change="handleFileUpload"
        accept=".jpg,.jpeg,.png,.webp,.bmp,.svg,.tif,.gif"
        style="display: none"
      />
    </MenuItem>

    <!-- 场景排序面板（浮层） -->
    <div
      v-if="showSortModal"
      class="fixed inset-0 z-[9998] flex items-center justify-center bg-black/60 p-4
        backdrop-blur-sm"
    >
      <div
        class="relative flex max-h-[85vh] w-full max-w-3xl flex-col overflow-hidden rounded-3xl
          border border-white/20 bg-slate-900/60 shadow-2xl backdrop-blur-2xl"
        @click.stop
      >
        <div class="flex items-center justify-between border-b border-white/10 bg-white/10 p-4">
          <h3 class="text-lg font-bold text-white">{{ $t("settings.background.sort.title") }}</h3>
          <button
            class="rounded-full p-2 text-white/50 transition-colors hover:bg-red-500/20
              hover:text-white"
            @click="showSortModal = false"
          >
            <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        <div
          ref="sortListRef"
          class="flex-1 space-y-2 overflow-y-auto p-4"
          @mousemove="onSortMouseMove($event)"
          @mouseup="onSortMouseUp"
          @mouseleave="onSortMouseUp"
        >
          <div
            v-for="(item, index) in sortItems"
            :key="item.id"
            @mousedown="onSortMouseDown($event, index)"
            class="flex cursor-grab items-center gap-3 rounded-xl border border-white/10 bg-white/5
              p-2 select-none active:cursor-grabbing"
            :style="
              draggingIndex === index
                ? {
                    transform: 'scale(1.03)',
                    boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
                    zIndex: 10,
                    position: 'relative',
                  }
                : {}
            "
            :class="index === overIndex ? 'ring-2 ring-indigo-400' : ''"
          >
            <template
              v-if="
                index === 0 || isSortItemFavored(item) !== isSortItemFavored(sortItems[index - 1])
              "
            >
              <div
                class="row-span-1 flex w-full items-center gap-1 pb-1 text-xs font-bold
                  tracking-widest text-white/40 uppercase"
              >
                <Star v-if="isSortItemFavored(item)" :size="12" class="text-amber-400" />
                {{
                  isSortItemFavored(item)
                    ? $t("settings.background.sort.favoredZone")
                    : $t("settings.background.sort.unfavoredZone")
                }}
              </div>
            </template>
            <img
              v-if="item.background"
              :src="convertFileSrc(item.background)"
              class="pointer-events-none h-10 w-16 shrink-0 rounded-lg object-cover"
              :alt="item.scene_name"
            />
            <div
              v-else
              class="pointer-events-none flex h-10 w-16 shrink-0 items-center justify-center
                rounded-lg bg-black/40 text-white/20"
            >
              <Image :size="28" />
            </div>
            <Star
              v-if="isSortItemFavored(item)"
              :size="14"
              class="pointer-events-none shrink-0 text-amber-400"
            />
            <span class="pointer-events-none flex-1 truncate text-sm text-white/85">{{
              item.scene_name
            }}</span>
            <span class="pointer-events-none shrink-0 text-xs text-white/30">{{ index + 1 }}</span>
          </div>
          <div v-if="sortItems.length === 0" class="py-8 text-center text-sm text-white/40">
            {{ $t("settings.background.sort.empty") }}
          </div>
        </div>

        <div class="flex justify-end gap-2 border-t border-white/10 bg-white/5 p-3">
          <button
            class="rounded-full bg-white/10 px-4 py-1.5 text-sm font-bold text-white/70
              hover:bg-white/20"
            @click="showSortModal = false"
          >
            {{ $t("settings.background.sort.cancel") }}
          </button>
          <button
            class="rounded-full bg-indigo-500/80 px-4 py-1.5 text-sm font-bold text-white
              hover:bg-indigo-500"
            @click="handleSaveSortOrder"
          >
            {{ $t("settings.background.sort.save") }}
          </button>
        </div>
      </div>
    </div>

    <MenuItem :title="$t('settings.background.particle.title')" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="effect-list flex gap-4 overflow-x-auto pb-2">
        <Button type="big" :active="currentParticle === 'None'" @click="updateParticle(`None`)">{{
          $t("settings.background.particle.none")
        }}</Button>
        <Button
          type="big"
          :active="currentParticle === 'StarField'"
          @click="updateParticle(`StarField`)"
          >{{ $t("settings.background.particle.starField") }}</Button
        >
        <Button type="big" :active="currentParticle === 'Rain'" @click="updateParticle(`Rain`)">{{
          $t("settings.background.particle.rain")
        }}</Button>
        <Button
          type="big"
          :active="currentParticle === 'Sakura'"
          @click="updateParticle(`Sakura`)"
          >{{ $t("settings.background.particle.sakura") }}</Button
        >
        <Button type="big" :active="currentParticle === 'Snow'" @click="updateParticle(`Snow`)">{{
          $t("settings.background.particle.snow")
        }}</Button>
        <Button
          type="big"
          :active="currentParticle === 'Fireworks'"
          @click="updateParticle(`Fireworks`)"
          >{{ $t("settings.background.particle.fireworks") }}</Button
        >
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.animation.switchTitle')" size="large">
      <template #header>
        <Settings :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <Toggle
          :checked="mainMenuStarsEnabled"
          @change="settingsStore.setMainMenuStarsEnabled($event)"
        >
          {{ $t("settings.background.animation.mainMenuStars") }}
        </Toggle>
        <Toggle
          :checked="mainMenuMeteorsEnabled"
          @change="settingsStore.setMainMenuMeteorsEnabled($event)"
        >
          {{ $t("settings.background.animation.mainMenuMeteors") }}
        </Toggle>
        <Toggle
          :checked="globalMouseTrailEnabled"
          @change="settingsStore.setGlobalMouseTrailEnabled($event)"
        >
          {{ $t("settings.background.animation.mouseTrail") }}
        </Toggle>
        <Toggle
          :checked="clickAnimationEnabled"
          @change="settingsStore.setClickAnimationEnabled($event)"
        >
          {{ $t("settings.background.animation.clickAnimation") }}
        </Toggle>
        <Toggle
          :checked="sceneAwarenessEnabled"
          @change="settingsStore.setSceneAwarenessEnabled($event)"
        >
          {{ $t("settings.background.animation.sceneAwareness") }}
        </Toggle>
      </div>
    </MenuItem>

    <!-- HDR 模式（仅 Windows：WebView2 强制色彩配置在 HDR 下会发灰/发暗） -->
    <MenuItem v-if="isWindows()" :title="$t('settings.background.hdr.title')" size="large">
      <template #header>
        <Settings :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <Toggle :checked="hdrModeEnabled" @change="settingsStore.setHdrModeEnabled($event)">
          {{ $t("settings.background.hdr.enable") }}
        </Toggle>
        <p class="text-xs text-yellow-400/70">
          {{ $t("settings.background.hdr.restartHint") }}
        </p>
        <button
          class="self-start rounded-lg border border-amber-500/30 bg-amber-500/20 px-4 py-2 text-sm
            font-medium text-amber-300 transition-colors hover:bg-amber-500/30
            disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!hdrChanged"
          @click="restartApp"
        >
          {{ $t("settings.background.hdr.restartBtn") }}
        </button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.animation.settingsTitle')" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="flex flex-col gap-4 p-2">
        <div
          class="flex items-center gap-4 max-[640px]:flex-col max-[640px]:items-stretch
            max-[640px]:gap-2"
        >
          <span class="min-w-30 text-sm font-medium text-white/90">{{
            $t("settings.background.animation.meteorFps")
          }}</span>
          <Slider
            v-model="meteorFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#8b5cf6"
            @change="handleMeteorFpsChange"
            class="flex-1"
          >
            <template #left>{{ meteorFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="meteorFpsInput"
            @blur="handleInputBlur"
            @keyup.enter="handleInputEnter"
            min="10"
            max="300"
            class="w-20 rounded-lg border border-white/20 bg-white/10 px-3 py-1.5 text-sm
              font-medium text-white transition-all focus:border-transparent focus:ring-2
              focus:ring-purple-500 focus:outline-none"
          />
        </div>

        <div
          class="flex items-center gap-4 max-[640px]:flex-col max-[640px]:items-stretch
            max-[640px]:gap-2"
        >
          <span class="min-w-30 text-sm font-medium text-white/90">{{
            $t("settings.background.animation.starsFps")
          }}</span>
          <Slider
            v-model="starsFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#fbbf24"
            @change="handleStarsFpsChange"
            class="flex-1"
          >
            <template #left>{{ starsFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="starsFpsInput"
            @blur="handleStarsInputBlur"
            @keyup.enter="handleStarsInputEnter"
            min="10"
            max="300"
            class="w-20 rounded-lg border border-white/20 bg-white/10 px-3 py-1.5 text-sm
              font-medium text-white transition-all focus:border-transparent focus:ring-2
              focus:ring-yellow-500 focus:outline-none"
          />
        </div>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.perf.title')" size="large">
      <template #header>
        <Cpu :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <!-- 加载中 -->
        <div v-if="perfLoading" class="flex items-center gap-2 text-sm text-white/60">
          <span
            class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/30
              border-t-white/80"
          ></span>
          {{ $t("settings.background.perf.detecting") }}
        </div>

        <!-- 检测结果 -->
        <div v-else-if="cpuInfo" class="flex flex-col gap-2">
          <!-- 未知 CPU 提示 -->
          <div
            v-if="cpuInfo.is_unknown && cpuInfo.unknown_message"
            class="flex items-center gap-2 rounded-lg border border-yellow-500/30 bg-yellow-500/15
              px-3 py-2 text-sm text-yellow-200"
          >
            <span>⚠️ {{ cpuInfo.unknown_message }}</span>
          </div>

          <!-- GPU 分级不适用 / 未检测到 GPU 提示 -->
          <div
            v-if="gpuInfo?.message"
            class="flex items-center gap-2 rounded-lg border border-yellow-500/30 bg-yellow-500/15
              px-3 py-2 text-sm text-yellow-200"
          >
            <span>⚠️ {{ gpuInfo.message }}</span>
          </div>

          <!-- CPU 行 -->
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.cpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{
              cpuInfo.brand
            }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(cpuInfo.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(cpuInfo.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(cpuInfo.tier as PerfTier) }}
            </span>
          </div>

          <!-- GPU 行（分级适用且有检测到 GPU 时显示） -->
          <div v-if="gpuInfo?.is_applicable && gpuInfo.name" class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.gpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{ gpuInfo.name }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(gpuInfo.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(gpuInfo.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(gpuInfo.tier as PerfTier) }}
            </span>
          </div>

          <!-- 当前实际调用的 GPU（WebGL 渲染器，反映程序真正在用的卡） -->
          <div v-if="activeGpu?.is_applicable && activeGpu.name" class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.activeGpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{
              activeGpu.name
            }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(activeGpu.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(activeGpu.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(activeGpu.tier as PerfTier) }}
            </span>
          </div>

          <!-- 综合等级（取最低） -->
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.combinedTier")
            }}</span>
            <span
              v-if="combinedTier"
              class="rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(combinedTier)"
              :style="{ backgroundColor: getPerfTierColor(combinedTier) + '99' }"
            >
              {{ getTierLabel(combinedTier) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.suggestedFps")
            }}</span>
            <span class="text-sm text-white/70">{{ suggestedFps }} FPS</span>
          </div>
        </div>

        <!-- 错误状态 -->
        <div v-else-if="perfError" class="text-sm text-red-300">
          {{ perfError }}
        </div>

        <!-- 重新检测按钮（同时重新检测 CPU 与 GPU） -->
        <button
          class="bg-brand/80 border-brand hover:bg-brand mt-1 self-start rounded-full border px-4
            py-1.5 text-sm font-bold text-white shadow-lg shadow-indigo-500/20 transition-all
            disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="perfLoading"
          @click="handleRedetectPerf"
        >
          {{
            perfLoading
              ? $t("settings.background.perf.detectingShort")
              : $t("settings.background.perf.redetect")
          }}
        </button>
      </div>
    </MenuItem>

    <!-- ========== 对话框外观（自定义） ========== -->
    <MenuItem :title="$t('settings.background.dialog.title')" size="large">
      <template #header>
        <MessageSquare :size="20" />
      </template>
      <DialogAppearancePanel />
    </MenuItem>

    <SceneEditModal
      :show="showSceneEdit"
      :mode="editMode"
      :backgrounds="backgroundList"
      :initial-data="editInitialData"
      @close="showSceneEdit = false"
      @submit="handleSceneSubmit"
      @upload="triggerUpload"
    />
  </MenuPage>
</template>

<script setup lang="ts">
  import { ref, onMounted, watch, computed } from "vue";
  import { useI18n } from "vue-i18n";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { MenuPage, MenuItem } from "../../ui";
  import { Button, Toggle, Slider } from "../../base";
  import { useGameStore } from "../../../stores/modules/game";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { useSettingsStore } from "../../../stores/modules/settings";
  import { isAndroid, isWindows } from "@/utils/platform";
  import { relaunch } from "@tauri-apps/plugin-process";
  import {
    listScenes,
    createScene,
    updateScene,
    deleteScene,
    selectScene,
    clearEmptyScenes,
    moveSceneToCategory,
    type SceneInfo,
    type LightingParams,
  } from "../../../api/services/scene";
  import type { BackgroundImageInfo } from "../../../types";
  import {
    getBackgroundImages,
    uploadBackgroundImage,
    generateBackgroundImage,
    openBackgroundsFolder,
    listBackgroundCategories,
    createBackgroundCategory,
    deleteBackgroundCategory,
  } from "../../../api/services/background";
  import { unlockAchievement } from "../../../api/services/achievement";
  import {
    getCpuInfo,
    redetectCpu,
    getTierLabel,
    getSuggestedMaxFps,
    getPerfTierColor,
    getCombinedTier,
    type CpuInfo,
    type PerfTier,
  } from "../../../api/services/cpu-perf";
  import {
    getGpuInfo,
    redetectGpu,
    getActiveGpu,
    type GpuInfo,
  } from "../../../api/services/gpu-perf";
  import {
    Image,
    PictureInPicture,
    Sparkles,
    Settings,
    Wand2,
    Wrench,
    Cpu,
    Star,
  } from "lucide-vue-next";
  import SceneEditModal from "../scene/SceneEditModal.vue";
  import DialogAppearancePanel from "../dialog/DialogAppearancePanel.vue";
  import PluginTag from "@/components/ui/PluginTag.vue";
  import { useUserStore } from "../../../stores/modules/user/user";

  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const settingsStore = useSettingsStore();
  const userStore = useUserStore();
  const dialogStore = useDialogStore();
  const { t } = useI18n();

  const mainMenuStarsEnabled = computed(() => settingsStore.mainMenuStarsEnabled);
  const mainMenuMeteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled);
  const globalMouseTrailEnabled = computed(() => settingsStore.globalMouseTrailEnabled);
  const clickAnimationEnabled = computed(() => settingsStore.clickAnimationEnabled);
  const sceneAwarenessEnabled = computed(() => settingsStore.sceneAwarenessEnabled);
  const hdrModeEnabled = computed(() => settingsStore.hdrModeEnabled);
  const currentParticle = computed(() => settingsStore.backgroundEffect);

  // 记录进入设置页时的初始值；开关改变后「立即重启」按钮才可用，改回原值则恢复置灰
  const initialHdrMode = ref(settingsStore.hdrModeEnabled);
  const hdrChanged = computed(() => settingsStore.hdrModeEnabled !== initialHdrMode.value);

  // 立即重启应用（HDR 模式等设置需重启后生效）
  async function restartApp() {
    const ok = await dialogStore.confirm(t("settings.background.hdr.restartConfirm"));
    if (!ok) return;
    try {
      await relaunch();
    } catch (e) {
      console.error("重启失败:", e);
      dialogStore.alert(t("settings.background.hdr.restartFailed"));
    }
  }
  const meteorFps = computed({
    get: () => settingsStore.meteorFps,
    set: (value: number) => {
      const clampedValue = Math.max(10, Math.min(60, value));
      settingsStore.setMeteorFps(clampedValue);
    },
  });
  const meteorFpsInput = ref(settingsStore.meteorFps);

  const starsFps = computed({
    get: () => settingsStore.starsFps,
    set: (value: number) => {
      const clampedValue = Math.max(10, Math.min(60, value));
      settingsStore.setStarsFps(clampedValue);
    },
  });
  const starsFpsInput = ref(settingsStore.starsFps);

  const backgroundList = ref<BackgroundImageInfo[]>([]);
  const uploadInput = ref<HTMLInputElement | null>(null);

  // ── 硬件性能检测（CPU + GPU） ──
  const cpuInfo = ref<CpuInfo | null>(null);
  const gpuInfo = ref<GpuInfo | null>(null);
  const activeGpu = ref<GpuInfo | null>(null);
  const perfLoading = ref(true);
  const perfError = ref<string | null>(null);

  /** 性能等级徽章样式 */
  function tierBadgeClassFor(tier: PerfTier): string {
    switch (tier) {
      case "Internet":
        return "bg-gray-500/60 text-gray-100";
      case "Low":
        return "bg-yellow-600/60 text-yellow-100";
      case "Medium":
        return "bg-blue-500/60 text-blue-100";
      case "High":
        return "bg-green-500/60 text-green-100";
      default:
        return "bg-white/20 text-white/60";
    }
  }

  /** 综合性能等级（取最低；GPU 分级不适用时仅按 CPU）。GPU 取「当前调用」优先，回退「最高性能」。 */
  const combinedTier = computed<PerfTier | null>(() => {
    if (!cpuInfo.value) return null;
    const cpuTier = cpuInfo.value.tier as PerfTier;
    const activeTier =
      activeGpu.value?.is_applicable && activeGpu.value.name
        ? (activeGpu.value.tier as PerfTier)
        : null;
    const maxTier = gpuInfo.value?.is_applicable ? (gpuInfo.value.tier as PerfTier) : null;
    const gpuTier = activeTier ?? maxTier;
    return getCombinedTier(cpuTier, gpuTier);
  });

  const suggestedFps = computed(() =>
    combinedTier.value ? getSuggestedMaxFps(combinedTier.value) : 30
  );

  const scenes = ref<SceneInfo[]>([]);

  // ── 场景收藏 + 手动排序（localStorage 持久化）──
  const SCENE_FAVORED_KEY = "lingchat.scene.favored.v1";
  const SCENE_ORDER_KEY = "lingchat.scene.order.v1";

  function loadSceneOrder(): string[] {
    try {
      const raw = localStorage.getItem(SCENE_ORDER_KEY);
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch {
      return [];
    }
  }
  function saveSceneOrderToStorage(order: string[]): void {
    try {
      localStorage.setItem(SCENE_ORDER_KEY, JSON.stringify(order));
    } catch {
      // ignore
    }
  }
  function loadFavored(): string[] {
    try {
      const raw = localStorage.getItem(SCENE_FAVORED_KEY);
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch {
      return [];
    }
  }
  function saveFavoredToStorage(favored: string[]): void {
    try {
      localStorage.setItem(SCENE_FAVORED_KEY, JSON.stringify(favored));
    } catch {
      // ignore
    }
  }
  // 显示顺序 = [收藏场景(按 favored 顺序)] + [未收藏场景(按 scene.order 相对顺序)]
  function applySceneOrder(list: SceneInfo[]): SceneInfo[] {
    const favored = loadFavored();
    const order = loadSceneOrder();
    const favoredSet = new Set(favored);
    const favoredScenes = favored
      .map((id) => list.find((s) => s.id === id))
      .filter((s): s is SceneInfo => !!s);
    const unfavoredScenes = list.filter((s) => !favoredSet.has(s.id));
    const orderIndex = (id: string) => {
      const i = order.indexOf(id);
      return i === -1 ? Number.MAX_SAFE_INTEGER : i;
    };
    unfavoredScenes.sort((a, b) => orderIndex(a.id) - orderIndex(b.id));
    return [...favoredScenes, ...unfavoredScenes];
  }
  function isFavored(sceneId: string): boolean {
    return loadFavored().includes(sceneId);
  }
  async function handleToggleFavorite(scene: SceneInfo): Promise<void> {
    let favored = loadFavored();
    if (favored.includes(scene.id)) {
      favored = favored.filter((id) => id !== scene.id);
      saveFavoredToStorage(favored);
      const order = loadSceneOrder();
      const nextOrder = [scene.id, ...order.filter((id) => id !== scene.id)];
      saveSceneOrderToStorage(nextOrder);
    } else {
      favored = [...favored, scene.id];
      saveFavoredToStorage(favored);
      const order = loadSceneOrder().filter((id) => id !== scene.id);
      saveSceneOrderToStorage(order);
    }
    scenes.value = applySceneOrder(scenes.value);
  }

  // ── 背景分类（子文件夹）+ 场景过滤 ──
  const backgroundCategories = ref<string[]>([]);
  const currentBackgroundCategory = ref<string>("全部");
  const newCategoryName = ref("");
  const VIRTUAL_CATEGORY = "插件";
  const isBackgroundCategoryReadOnly = computed(
    () => currentBackgroundCategory.value === VIRTUAL_CATEGORY
  );
  const writableBackgroundCategories = computed(() =>
    backgroundCategories.value.filter((category) => category !== VIRTUAL_CATEGORY)
  );

  // 场景的背景 → 所属分类 映射（url → category），用于按分类过滤场景卡片。
  // 先按完整 url 精确匹配；再按文件名（basename）兜底匹配，兼顾
  // 前后端对路径分隔符/大小写的表示差异，避免子分类标签下场景被误归为“根目录”。
  function categoryOfBackground(url: string): string {
    if (!url) return "根目录";
    const matched = backgroundList.value.find((b) => b.url === url);
    if (matched?.category) return matched.category;
    const base = (url.split(/[\\/]/).pop() || "").toLowerCase();
    const byName = backgroundList.value.find((b) => {
      const bBase = (b.url || "").split(/[\\/]/).pop() || "";
      return bBase.toLowerCase() === base;
    });
    return byName?.category || "根目录";
  }

  // 收藏置顶排序后的完整场景列表（再按背景分类过滤）
  const orderedScenes = computed(() => applySceneOrder(scenes.value));
  // 按当前选中的背景分类过滤场景
  const filteredScenes = computed(() => {
    if (!currentBackgroundCategory.value || currentBackgroundCategory.value === "全部") {
      return orderedScenes.value;
    }
    return orderedScenes.value.filter((s) => {
      const bgPath = s.background || "";
      return categoryOfBackground(bgPath) === currentBackgroundCategory.value;
    });
  });

  // 分页
  const ITEMS_PER_PAGE = 6;
  const currentPage = ref(1);
  const totalPages = computed(() =>
    Math.max(1, Math.ceil(filteredScenes.value.length / ITEMS_PER_PAGE))
  );
  const paginatedScenes = computed(() => {
    const start = (currentPage.value - 1) * ITEMS_PER_PAGE;
    return filteredScenes.value.slice(start, start + ITEMS_PER_PAGE);
  });
  // 场景或切分类变化时回到第一页
  watch([scenes, currentBackgroundCategory], () => {
    if (currentPage.value > totalPages.value) currentPage.value = totalPages.value;
  });

  const showSceneEdit = ref(false);
  const editMode = ref<"create" | "update">("create");
  const editingSceneId = ref<string | null>(null);
  const editInitialData = ref<
    | {
        sceneName: string;
        sceneImage: string | null;
        sceneDescription: string;
        lighting?: LightingParams | null;
      }
    | undefined
  >();

  const currentSceneDisplay = computed(
    () => gameStore.currentScene?.scene_name || t("settings.background.scene.none")
  );
  const currentScene = computed(() => gameStore.currentScene);

  const fetchScenes = async () => {
    try {
      scenes.value = await listScenes();
    } catch (error) {
      console.error("获取场景列表失败", error);
    }
  };

  const isSceneSelected = (sceneId: string): boolean => {
    return gameStore.currentScene?.id === sceneId;
  };

  const handleSceneClick = async (scene: SceneInfo) => {
    // 点击当前已激活的场景则取消选中，背景默认为透明
    if (gameStore.currentScene?.id === scene.id) {
      gameStore.clearCurrentScene();
      uiStore.setCurrentBackground("");
      unlockAchievement("see_through").catch(console.error);
      await fetchScenes();
      return;
    }

    // 无描述时提醒用户
    if (!scene.scene_description?.trim()) {
      uiStore.showInfo({
        title: t("settings.background.scene.tip"),
        message: t("settings.background.scene.noDescriptionTip", { name: scene.scene_name }),
        duration: 4000,
      });
    }

    try {
      await selectScene(scene.id);
      gameStore.setCurrentScene(scene);
      if (scene.background) {
        uiStore.setCurrentBackground(scene.background);
      }
      await fetchScenes();
    } catch (error) {
      console.error("选择场景失败", error);
    }
  };

  const handleWrenchClick = (scene: SceneInfo) => {
    editMode.value = "update";
    editingSceneId.value = scene.id;
    editInitialData.value = {
      sceneName: scene.scene_name,
      sceneImage: scene.background || null,
      sceneDescription: scene.scene_description,
      lighting: scene.lighting,
    };
    showSceneEdit.value = true;
  };

  const handleCreateScene = () => {
    editMode.value = "create";
    editingSceneId.value = null;
    editInitialData.value = undefined;
    showSceneEdit.value = true;
  };

  const handleDeleteScene = async () => {
    if (!currentScene.value) return;
    if (currentScene.value.source && currentScene.value.source !== "game") {
      await dialogStore.alert(t("settings.background.scene.pluginNotDeletable"));
      return;
    }
    if (
      !(await dialogStore.confirm(
        t("settings.background.scene.deleteConfirm", { name: currentScene.value.scene_name })
      ))
    )
      return;

    try {
      await deleteScene(currentScene.value.id);
      gameStore.clearCurrentScene();
      await fetchScenes();
    } catch (error) {
      console.error("删除场景失败", error);
    }
  };

  const handleSceneSubmit = async (data: {
    sceneName: string;
    sceneImage: string | null;
    sceneDescription: string;
    lighting?: LightingParams | null;
  }) => {
    try {
      if (editMode.value === "create") {
        await createScene({
          scene_name: data.sceneName,
          scene_description: data.sceneDescription,
          background: data.sceneImage || "",
          lighting: data.lighting ?? null,
        });
      } else {
        if (!editingSceneId.value) return;
        await updateScene({
          id: editingSceneId.value,
          scene_name: data.sceneName,
          scene_description: data.sceneDescription,
          background: data.sceneImage || "",
          lighting: data.lighting ?? null,
        });
      }
      showSceneEdit.value = false;
      await fetchScenes();

      // 如果更新的是当前选中的场景，立即同步到 gameStore 使光影等参数即时生效
      if (editMode.value === "update" && editingSceneId.value === gameStore.currentScene?.id) {
        const updatedScene = scenes.value.find((s) => s.id === editingSceneId.value);
        if (updatedScene) {
          gameStore.setCurrentScene(updatedScene);
          if (updatedScene.background) {
            uiStore.setCurrentBackground(updatedScene.background);
          }
        }
      }
    } catch (error) {
      console.error("操作失败", error);
    }
  };

  onMounted(async () => {
    try {
      await refreshBackground();
    } catch (error) {
      console.error("加载背景图片失败", error);
    }

    await fetchScenes();

    // 恢复上次选中的场景
    if (gameStore.currentScene?.background) {
      uiStore.setCurrentBackground(gameStore.currentScene.background);
    }

    // 加载 CPU + GPU 性能信息
    await fetchPerfInfo();
  });

  // ── 硬件性能检测（CPU + GPU） ──

  async function fetchPerfInfo(): Promise<void> {
    perfLoading.value = true;
    perfError.value = null;
    try {
      // activeGpu 读取真实 WebGL 渲染器（反映当前实际调用的 GPU），失败不影响其余信息
      const [cpu, gpu, active] = await Promise.all([
        getCpuInfo(),
        getGpuInfo(),
        getActiveGpu().catch(() => null),
      ]);
      cpuInfo.value = cpu;
      gpuInfo.value = gpu;
      activeGpu.value = active;
    } catch (e: any) {
      perfError.value = e?.message || t("settings.background.perf.fetchFailed");
      console.error("获取硬件性能信息失败", e);
    } finally {
      perfLoading.value = false;
    }
  }

  async function handleRedetectPerf(): Promise<void> {
    perfLoading.value = true;
    perfError.value = null;
    try {
      const [cpu, gpu, active] = await Promise.all([
        redetectCpu(),
        redetectGpu(),
        getActiveGpu().catch(() => null),
      ]);
      cpuInfo.value = cpu;
      gpuInfo.value = gpu;
      activeGpu.value = active;
      uiStore.showSuccess({
        title: t("settings.background.perf.detectComplete"),
        message: t("settings.background.perf.tierMessage", {
          tier: combinedTier.value
            ? getTierLabel(combinedTier.value)
            : t("settings.background.perf.unknown"),
        }),
        duration: 3000,
      });
    } catch (e: any) {
      perfError.value = e?.message || t("settings.background.perf.redetectFailed");
      console.error("重新检测硬件性能失败", e);
    } finally {
      perfLoading.value = false;
    }
  }

  async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
    try {
      const data = await getBackgroundImages();
      return data.map((background: BackgroundImageInfo) => ({
        title: background.title || "Untitled",
        url: background.url || "",
        time: background.time,
        // 保留所属子分类（子文件夹名），否则按分类选项卡过滤场景时会全部落到“根目录”
        category: background.category,
      }));
    } catch (error) {
      console.error("Failed to fetch background list:", error);
      return [];
    }
  }

  async function refreshBackground(): Promise<void> {
    const items = await fetchBackgrounds();
    backgroundList.value = items;
    await loadBackgroundCategories();
  }

  async function loadBackgroundCategories(): Promise<void> {
    try {
      const cats = await listBackgroundCategories();
      backgroundCategories.value = cats;
      if (
        currentBackgroundCategory.value !== "全部" &&
        !cats.includes(currentBackgroundCategory.value)
      ) {
        currentBackgroundCategory.value = "全部";
      }
    } catch (error) {
      console.error("加载背景分类失败", error);
    }
  }

  async function handleCreateCategory(): Promise<void> {
    const name = newCategoryName.value.trim();
    if (!name) {
      await dialogStore.alert(t("settings.background.scene.categoryNameEmpty"));
      return;
    }
    try {
      await createBackgroundCategory(name);
      newCategoryName.value = "";
      await refreshBackground();
      await fetchScenes();
      uiStore.showSuccess({
        title: t("settings.background.scene.categoryCreated"),
        message: t("settings.background.scene.categoryCreatedMsg", { name }),
        duration: 3000,
      });
    } catch (error: any) {
      console.error("创建分类失败:", error);
      await dialogStore.alert(t("settings.background.scene.categoryCreateFail"));
    }
  }

  async function handleDeleteCategoryFlow(): Promise<void> {
    const cat = currentBackgroundCategory.value;
    if (!cat || cat === "全部" || cat === VIRTUAL_CATEGORY) return;
    const confirmed = await dialogStore.confirm(
      t("settings.background.scene.categoryDeleteConfirmMove", { name: cat })
    );
    if (!confirmed) return;
    try {
      const result = await deleteBackgroundCategory(cat, "move_to_root");
      await refreshBackground();
      await fetchScenes();
      currentBackgroundCategory.value = "全部";
      uiStore.showSuccess({
        title: t("settings.background.scene.categoryDeleted"),
        message: t("settings.background.scene.categoryDeletedMoved", { name: cat, count: result }),
        duration: 3000,
      });
    } catch (error) {
      console.error("删除分类失败:", error);
      await dialogStore.alert(t("settings.background.scene.categoryDeleteFail"));
    }
  }

  function triggerUpload(): void {
    if (isBackgroundCategoryReadOnly.value) return;
    uploadInput.value?.click();
  }

  async function handleFileUpload(event: Event): Promise<void> {
    if (isBackgroundCategoryReadOnly.value) return;
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    const fileName = file.name;
    const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();
    const allowedExts = [".jpg", ".jpeg", ".png", ".webp", ".bmp", ".svg", ".tif", ".gif"];

    if (!allowedExts.includes(fileExt)) {
      await dialogStore.alert(
        t("settings.background.upload.invalidFormat", { formats: allowedExts.join(", ") })
      );
      return;
    }

    try {
      const buf = await file.arrayBuffer();
      const category =
        currentBackgroundCategory.value === "全部" ? undefined : currentBackgroundCategory.value;
      await uploadBackgroundImage(fileName, new Uint8Array(buf), category);
      await refreshBackground();
      // 刷新场景列表（后端会自动将新背景注册为场景）
      await fetchScenes();
      if (target) target.value = "";
    } catch (error) {
      console.error("上传失败", error);
      await dialogStore.alert(t("settings.background.upload.failed"));
    }
  }

  function updateParticle(value: string): void {
    uiStore.setBackgroundEffect(value);
  }

  async function handleOpenFolder(): Promise<void> {
    try {
      await openBackgroundsFolder();
    } catch (e: any) {
      uiStore.showError({
        title: t("settings.background.folder.errorTitle"),
        message: t("settings.background.folder.openFailed"),
      });
    }
  }

  // ── 场景排序面板（自研指针拖拽）──
  const showSortModal = ref(false);
  const sortItems = ref<SceneInfo[]>([]);
  const sortListRef = ref<HTMLElement | null>(null);
  const draggingIndex = ref(-1);
  const overIndex = ref(-1);

  async function openSortModal(): Promise<void> {
    // 全局收藏与排序键是跨分类的，在子分类下保存会用当前子集覆盖全局，
    // 因此只允许在「全部」分类下调整全局排序。
    if (currentBackgroundCategory.value !== "全部") {
      await dialogStore.alert(t("settings.background.sort.onlyAll"));
      return;
    }
    const all = filteredScenes.value;
    sortItems.value = all.map((s) => ({ ...s }));
    showSortModal.value = true;
  }

  function isSortItemFavored(item: SceneInfo): boolean {
    return loadFavored().includes(item.id);
  }

  function onSortMouseDown(event: MouseEvent, index: number): void {
    event.preventDefault();
    draggingIndex.value = index;
    overIndex.value = index;
  }

  function onSortMouseMove(event: MouseEvent): void {
    if (draggingIndex.value === -1) return;
    const list = sortListRef.value;
    if (!list) return;
    const items = Array.from(list.querySelectorAll<HTMLElement>(":scope > div"));
    const draggedFavored = isSortItemFavored(sortItems.value[draggingIndex.value]);
    let zoneBoundary = 0;
    sortItems.value.forEach((it, i) => {
      if (isSortItemFavored(it)) zoneBoundary = i + 1;
    });
    const pointerY = event.clientY;
    let target = draggingIndex.value;
    items.forEach((el, i) => {
      const rect = el.getBoundingClientRect();
      const mid = rect.top + rect.height / 2;
      if (pointerY > mid) target = i;
    });
    if (draggedFavored) {
      target = Math.max(0, Math.min(target, Math.max(0, zoneBoundary - 1)));
    } else {
      target = Math.max(zoneBoundary, Math.min(target, sortItems.value.length - 1));
    }
    if (target !== overIndex.value) {
      overIndex.value = target;
      const arr = [...sortItems.value];
      const [moved] = arr.splice(draggingIndex.value, 1);
      arr.splice(target, 0, moved);
      sortItems.value = arr;
      draggingIndex.value = target;
    }
  }

  function onSortMouseUp(): void {
    draggingIndex.value = -1;
    overIndex.value = -1;
  }

  async function handleSaveSortOrder(): Promise<void> {
    const favored = loadFavored();
    const favoredSet = new Set(favored);
    const newFavoredOrder = sortItems.value.filter((s) => favoredSet.has(s.id)).map((s) => s.id);
    const newOrder = sortItems.value.filter((s) => !favoredSet.has(s.id)).map((s) => s.id);
    if (newFavoredOrder.length > 0) saveFavoredToStorage(newFavoredOrder);
    if (newOrder.length > 0) saveSceneOrderToStorage(newOrder);
    showSortModal.value = false;
    scenes.value = applySceneOrder(scenes.value);
    uiStore.showSuccess({
      title: t("settings.background.sort.saved"),
      duration: 2000,
    });
  }

  // ── 刷新：删除空场景 + 刷新背景 + 刷新列表（合并为一个按钮）──
  async function handleRefreshScenes(): Promise<void> {
    const confirmed = await dialogStore.confirm(t("settings.background.scene.refreshConfirm"));
    if (!confirmed) return;
    try {
      const removed = await clearEmptyScenes();
      await refreshBackground();
      await fetchScenes();
      uiStore.showSuccess({
        title: t("settings.background.scene.refreshDone"),
        message: t("settings.background.scene.refreshDoneMsg"),
        duration: 3000,
      });
    } catch (e: any) {
      console.error("刷新场景失败:", e);
      uiStore.showError({
        title: t("settings.background.scene.refreshFail"),
        message: e?.message || "",
      });
    }
  }

  // ── 场景右键菜单：移动到子分类 ──
  const sceneMenu = ref<{ visible: boolean; x: number; y: number; scene: SceneInfo | null }>({
    visible: false,
    x: 0,
    y: 0,
    scene: null,
  });

  // 菜单定位：尽量不超出视口右/下边界
  const sceneMenuStyle = computed(() => ({
    left: Math.max(0, Math.min(sceneMenu.value.x, window.innerWidth - 190)) + "px",
    top: Math.max(0, Math.min(sceneMenu.value.y, window.innerHeight - 260)) + "px",
  }));

  function openSceneContextMenu(scene: SceneInfo, event: MouseEvent): void {
    if (scene.source && scene.source !== "game") return;
    sceneMenu.value = { visible: true, x: event.clientX, y: event.clientY, scene };
  }

  function closeSceneContextMenu(): void {
    sceneMenu.value.visible = false;
  }

  // 把场景的背景图片移动到目标子分类（子文件夹）
  async function handleMoveScene(scene: SceneInfo, category: string): Promise<void> {
    closeSceneContextMenu();
    if (!scene.background) {
      await dialogStore.alert(t("settings.background.scene.moveNoBackground"));
      return;
    }
    try {
      await moveSceneToCategory(scene.id, category);
      await refreshBackground();
      await fetchScenes();
      uiStore.showSuccess({
        title: t("settings.background.scene.movedTitle"),
        message: t("settings.background.scene.movedMsg", { name: scene.scene_name, category }),
        duration: 3000,
      });
    } catch (e: any) {
      console.error("移动场景到分类失败:", e);
      uiStore.showError({
        title: t("settings.background.scene.moveFail"),
        message: e?.message || "",
      });
    }
  }

  function handleMeteorFpsChange(value: number) {
    const clampedValue = Math.max(10, Math.min(60, value));
    meteorFpsInput.value = clampedValue;
    settingsStore.setMeteorFps(clampedValue);
  }

  function handleInputBlur() {
    let value = Number(meteorFpsInput.value);
    if (isNaN(value) || value < 10) value = 10;
    else if (value > 300) value = 300;
    meteorFpsInput.value = value;
    settingsStore.setMeteorFps(value);
  }

  function handleInputEnter() {
    handleInputBlur();
  }

  watch(meteorFps, (newValue) => {
    meteorFpsInput.value = newValue;
  });

  function handleStarsFpsChange(value: number) {
    const clampedValue = Math.max(10, Math.min(60, value));
    starsFpsInput.value = clampedValue;
    settingsStore.setStarsFps(clampedValue);
  }

  function handleStarsInputBlur() {
    let value = Number(starsFpsInput.value);
    if (isNaN(value) || value < 10) value = 10;
    else if (value > 300) value = 300;
    starsFpsInput.value = value;
    settingsStore.setStarsFps(value);
  }

  function handleStarsInputEnter() {
    handleStarsInputBlur();
  }

  watch(starsFps, (newValue) => {
    starsFpsInput.value = newValue;
  });

  // ── 对话框外观 ──
  const dialogBgInput = ref<HTMLInputElement | null>(null);

  function hexToRgba(hex: string, alpha: number): string {
    const m = hex.replace("#", "").match(/^([0-9a-fA-F]{6})$/);
    if (!m) return `rgba(0,14,39,${alpha})`;
    const r = parseInt(m[1]!.substring(0, 2), 16);
    const g = parseInt(m[1]!.substring(2, 4), 16);
    const b = parseInt(m[1]!.substring(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
</script>

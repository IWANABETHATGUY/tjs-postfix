import { client, fetchBackContext } from '@/plugins/client-start';
import api, {
  clearTrash,
  batchMoveFiles,
  batchCopyFiles
} from '@/api/drive.ts';

import Vue from 'vue';
import { updateListByPropId, log, checkCanPreview } from '@/utils/util';
import chunk from 'lodash/chunk';
import { ActionContext, MutationTree, ActionTree } from 'vuex';
import { Dictionary } from '@/utils/type';
import bus from '@/utils/eventBus';
import { createFetchBackBW, isFetchBackBWOpen } from '@/utils/remote';
import { SortFunctionMapType, SortOrderType, sortFunctionMap, isTwoOrderedListEqual } from '@/utils/drive-util';
import { getConfig, callRemote, redirectCurrentDirectoryById, openImgView, NavViewID } from '@/utils/IPC';

import { TaskType } from '@/utils/download-kernel';
import { eventTrack } from '@/utils/event-track';
import { FileTreeSqlManager, sqlColorfulLogger } from '@/sql/fileTree/db';
import userFileSqlApi from '@/sql/fileTree/api';

import { createTaskInfo } from '@/utils/retrieval/type';
import { getFreePartitionSpace } from '@/utils/thunderHelper';
import { IPlayerOption } from './media-player';
import staticIcons from '@/utils/static-icons';
// 这个标识解决了，当用户频繁打开文件夹， 通常本地数据库返回会比 http 请求快， 所以就会出现用户进入了几次文件夹，
// 同步的结果可能才返回，用timestamp 可以让过时的sync 丢弃, 避免视图闪烁
let lastSyncFileListTimeStamp = Date.now();

export const initState = (): DriveState => ({
  cover: false,
  activeCollapseItemType: 'drive',
  all: {},
  home: {
    list: []
  },
  uploadList: [], // 当前选择的上传的文件
  tasks: {
    list: [],
    pageToken: ''
  },
  allTaskStatus: {},
  spaceMsg: {
    kind: '',
    limit: 10,
    usage: 0
  },
  pageToken: '',
  hasMore: false,
  parentId: '',
  parentPaths: [
    {
      id: '',
      name: '全部文件'
    }
  ], // 目录路径
  driveFileList: [],
  trashFileList: [],
  driveRouteList: [{ title: '我的云盘', id: '' }],
  nextPageTokenMap: {},
  driveSortInfoMap: {
    drive: {
      type: 'created_time',
      order: 1
    },
    trash: {
      type: 'modified_time',
      order: -1
    }
  },
  parentIdMap: {
    drive: '',
    trash: '*'
  },
  selectItemListMap: {
    drive: [],
    trash: []
  },
  parentIndex: 0,
  folders: {
    0: {
      id: '',
      name: '根目录'
    }
  }, // 目录树
  folderNodes: [],
  events: {
    list: [],
    pageToken: ''
  },
  uploadRetry: false,
  filterTypes: {
    all: { name: '全部文件', filter: { trashed: { eq: false } } },
    done: {
      name: '已完成',
      filter: { phase: { eq: 'PHASE_TYPE_COMPLETE' }, trashed: { eq: false } }
    },
    starred: { name: '加星', filter: { starred: { eq: true } } },
    file: {
      name: '文件',
      filter: { kind: { eq: 'drive#file' }, trashed: { eq: false } }
    },
    category: {
      name: '目录',
      filter: { kind: { eq: 'drive#folder' }, trashed: { eq: false } }
    },
    picture: {
      name: '图片',
      filter: { mime_type: { prefix: 'image/' }, trashed: { eq: false } }
    },
    // {name: '文档', filter: {}},
    video: {
      name: '视频',
      filter: { mime_type: { prefix: 'video/' }, trashed: { eq: false } }
    },
    trash: { name: '回收站', filter: { trashed: { eq: true } } }
  },
  curFilter: 'all',
  importState: {
    isImported: false,
    canLoadRecords: 0
  },
  toFetchBackList: [],
  shareInfo: {
    shareId: '',
    passCodeToken: ''
  },
  userFileTableColumnList: []
});

export const state = initState();

export const mutations: MutationTree<DriveState> = {
  set (state, file: { id: string | number }) {
    Vue.set(state.all, file.id, Object.assign({}, state.all[file.id], file));
  },
  setFiles (state: DriveState, { list, refresh = false }: Dictionary): void {
    if (refresh) {
      state.home.list = list;
    } else {
      state.home.list = [...state.home.list, ...list];
    }
  },
  // 添加 refreshSort 是为了再 创建文件夹的情况时
  setDriveFileList (
    state,
    {
      data,
      refresh = false,
      position = 'end',
      sort = true
    }: {
      data: API_FILE.DriveFile[];
      refresh: boolean;
      position: 'start' | 'end';
      sort: boolean;
    }
  ) {
    if (refresh) {
      Vue.set(state, 'driveFileList', data);
    } else {
      if (position === 'start') {
        state.driveFileList.unshift(...data);
      } else {
        state.driveFileList.push(...data);
      }
    }
    sort && mutations.sortFileList(state, 'drive');
  },
  setDriveSortInfo (state, { tabType, info }: { tabType: CollapseItemTabType; info: TypeDriveSortInfo }) {
    const currentInfo = state.driveSortInfoMap[tabType];
    state.driveSortInfoMap[tabType] = { ...currentInfo, ...info };
  },
  sortFileList (state, type: CollapseItemTabType) {
    console.log('sort', type);
    const sortInfo = state.driveSortInfoMap[type];
    if (type === 'trash') {
      state.trashFileList.sort(sortFunctionMap[sortInfo.type](sortInfo.order));
    } else {
      state.driveFileList.sort(sortFunctionMap[sortInfo.type](sortInfo.order));
    }
  },
  setTrashFileList (state, { data, refresh = false }) {
    if (refresh) {
      Vue.set(state, 'trashFileList', data);
    } else {
      state.trashFileList.push(...data);
    }
    mutations.sortFileList(state, 'trash');
  },
  clearFileMode (state, { type }: { type: CollapseItemTabType }) {
    if (type === 'drive') {
      state.driveFileList.forEach((item: any) => {
        item.__mode = '';
      });
    }
  },
  replaceFileItem (state, { item, type }: { item: API_FILE.DriveFile; type: CollapseItemTabType }) {
    if (type === 'drive') {
      const index = state.driveFileList.findIndex(file => item.id === file.id);
      if (index >= 0) {
        Vue.set(state.driveFileList, index, item);
      }
    }
  },
  setActiveCollapseTabType (state, value: CollapseItemTabType) {
    state.activeCollapseItemType = value;
  },
  setSelectedItemList (state, { type, list }: { type: CollapseItemTabType; list: API_FILE.DriveFile[] }) {
    Vue.set(state.selectItemListMap, type, list);
  },
  setNextPageTokenMap (state, pageTokenMap: any) {
    state.nextPageTokenMap = { ...state.nextPageTokenMap, ...pageTokenMap };
  },
  setDriveRouteList (state, newRouteList: any[]) {
    // 仅在路径发生变化的时候设置  driveRouteList
    if (state.driveRouteList.slice(-1)[0].id !== newRouteList.slice(-1)[0].id) {
      Vue.set(state, 'driveRouteList', newRouteList);
    }
  },
  setParentIdMap (state, parentIdMap: any) {
    state.parentIdMap = { ...state.parentIdMap, ...parentIdMap };
  },

  addDirectory (state: DriveState, { list }: Dictionary) {
    state.home.list = [...list, ...state.home.list];
  },
  add (state: DriveState, file: any): void {
    state.home.list.unshift(file);
  },
  // 这里的 mutation `deleteFile` 意思是说从某一个列表中把目标为id 的item 删除
  deleteFile (state: DriveState, { id, type }: { id: string | number; type: CollapseItemTabType }): void {
    let targetFileList: any[] = [];
    if (type === 'drive') {
      targetFileList = state.driveFileList;
    } else {
      targetFileList = state.trashFileList;
    }
    targetFileList.forEach((item: any, index) => {
      if (item.id === id) {
        targetFileList.splice(index, 1);
      }
    });
  },
  batchDeleteFiles (state: DriveState, { ids, type }: { ids: string[]; type: CollapseItemTabType }): void {
    const typeMap: { [key in CollapseItemTabType]: keyof Pick<DriveState, 'driveFileList' | 'trashFileList'> } = {
      drive: 'driveFileList',
      trash: 'trashFileList'
    };
    const filterRes = state[typeMap[type]].filter(file => {
      if (file.id && ids.includes(file.id)) {
        return false;
      }
      return true;
    });
    Vue.set(state, typeMap[type], filterRes);
  },
  setUploadTask (state: DriveState, files: any): void {
    console.log('更新uplist', state.uploadList, files);
    if (Array.isArray(files)) {
      state.uploadList = [...state.uploadList, ...files];
    } else {
      // 单文件部分更新
      const uploadList = state.uploadList;
      let oldFile: object = {};
      let index = -1;
      for (let i = 0; i < uploadList.length; i++) {
        if (files.nativeFileUUID === uploadList[i].nativeFileUUID) {
          index = i;
          oldFile = uploadList[i];
        }
      }
      if (index < 0) {
        console.log('更新数据异常, 传入的数据在原有的记录中无法找到');
        return;
      }
      const newFile = Object.assign({}, oldFile, files);
      console.log('索引', index);
      console.log(state.uploadList.toString());

      state.uploadList.splice(index, 1, newFile);
    }
  },
  delUploadTask (state: DriveState, file: any): void {
    console.log('准备删除', state.uploadList, file);

    state.uploadList = state.uploadList.filter(v => v.nativeFileUUID !== file.nativeFileUUID);
  },
  setTasks (state: DriveState, { refresh, list, pageToken }: Dictionary): void {
    state.tasks = {
      list: refresh ? list : [...state.tasks.list, ...list],
      pageToken
    };
  },
  setTaskStatus (state: DriveState, { id, statuses }: { id: string; statuses: string[] }) {
    const list = state.tasks.list.map(v => {
      if (v.id === id) {
        return {
          ...v,
          statuses
        };
      } else {return v;};
    });
    state.tasks.list = list;
  },
  setSpageMsg (state: DriveState, res: any): void {
    state.spaceMsg = res;
  },
  delTasks (state: DriveState, ids: any): void {
    state.tasks.list = state.tasks.list.filter(v => !ids.includes(v.id));
  },
  setDiskInfo (state: DriveState, res: any): void {
    state.pageToken = res.pageToken;
    state.hasMore = res.hasMore;
    state.parentId = res.parentId;
  },
  setParentId (state: DriveState, id: string): void {
    state.parentId = id;
  },
  setFolders (state: DriveState, folder: any): void {
    Vue.set(state.folders, folder.id, folder);
  },
  setFolderNodes (state: DriveState, { parentId, list }: Dictionary): void {
    const oldList = [...state.folderNodes];
    if (parentId === '0') {
      state.folderNodes = [
        {
          id: '0',
          name: '根目录'
        },
        ...list
      ];
    } else {
      const newList = updateListByPropId(parentId, oldList, list);
      state.folderNodes = newList;
    }
  },
  setUserFileTableColumnList (state, list: string[]): void {
    state.userFileTableColumnList = list;
  },
  setFolder (state: DriveState, folder: any): void {
    state.parentPaths = [...state.parentPaths, folder];
  },
  backFolder (state: DriveState, index: number): void {
    state.parentPaths = state.parentPaths.slice(0, index);
  },
  setAllTaskStatus (state: DriveState, status: any): void {
    state.allTaskStatus[status.id] = status;
  },
  setCover (state: DriveState, bol: boolean): void {
    state.cover = bol;
  },
  setEvents (state: DriveState, { refresh, list, pageToken }: Dictionary): void {
    state.events = {
      list: refresh ? list : [...state.events.list, ...list],
      pageToken
    };
  },
  setUploadRetry (state: DriveState, bol: boolean): void {
    state.uploadRetry = bol;
  },
  setFilter (state: DriveState, key: string): void {
    state.curFilter = key;
  },
  setImportStatus (state: DriveState, importStatus: API_FILE.DriveCheckImportDownloadResponse): void {
    state.importState = {
      isImported: importStatus.is_import || false,
      canLoadRecords: importStatus.can_load_records || 0
    };
  },
  setFetchBackFiles (state: DriveState, files: createTaskInfo[]) {
    state.toFetchBackList = files;
  },
  setShareInfo (state: DriveState, { shareId = '', passCodeToken = '' }: {shareId: string; passCodeToken: string}) {
    state.shareInfo.shareId = shareId;
    state.shareInfo.passCodeToken = passCodeToken;
  },
  reset (state: DriveState) {
    Object.assign(state, initState());
  }
};

export const actions: ActionTree<DriveState, any> = {
  getFileList ({ state, commit, rootState, dispatch }, params: Dictionary = {}) {
    const type = params.__type || 'drive';
    const filters = type === 'drive' ? state.filterTypes.done.filter : state.filterTypes.trash.filter;
    return userFileSqlApi.findFiles({ ...params, filters }).then((res: any) => {
      if (res.files) {
        sqlColorfulLogger('user_file', 'getFileList/files', res.files);
        if (type === 'drive') {
          commit('setDriveFileList', { data: res.files, ...params });
        } else {
          commit('setTrashFileList', { data: res.files, ...params });
        }
        // commit('setNextPageTokenMap', { [type]: res.next_page_token })
        if (params.__sync) {
          const parent_id = params.parent_id;
          lastSyncFileListTimeStamp = Date.now();
          FileTreeSqlManager.directorySynchronize({ params, parent_id, type, timestamp: lastSyncFileListTimeStamp }).then(
            ([success, fileList, timestamp]) => {
              if (success && timestamp === lastSyncFileListTimeStamp) {
                if (type === 'drive') {
                  commit('setDriveFileList', { data: fileList, ...params });
                } else {
                  commit('setTrashFileList', { data: fileList, ...params });
                }

                const serverFileIdList = (fileList as API_FILE.DriveFile[]).map(item => item.id).filter(Boolean) as string[];
                const localFileIdList = (res.files as API_FILE.DriveFile[]).map(item => item.id).filter(Boolean) as string[];
                if (!isTwoOrderedListEqual(serverFileIdList, localFileIdList, (a, b) => a.localeCompare(b), (a, b) => a === b)) {
                  sqlColorfulLogger('user_file', 'diff_from_server', 'diff found');
                  FileTreeSqlManager.batchDeleteById(localFileIdList);
                  FileTreeSqlManager.batchInsertOrUpdate(fileList as API_FILE.DriveFile[]);
                }
              }
            }
          );
        }
      } else {
        throw new Error(JSON.stringify(res));
      }
      return res;
    });
  },
  addFileToList ({ state, commit }, id: string) {
    return api.getFileInfo(id).then(file => {
      if (file.parent_id === state.driveRouteList[state.driveRouteList.length - 1].id) {
        commit('setDriveFileList', { data: [file] });
      }
    });
  },

  getFileInfo ({ commit }: ActionContext<DriveState, DriveState>, id: any) {
    return api.getFileInfo(id).then(res => {
      commit('set', res);
      return res;
    });
  },
  deleteFile (
    { commit, dispatch }: ActionContext<DriveState, DriveState>,
    { id, type }: { id: string | number; type: CollapseItemTabType }
  ) {
    return api
      .deleteFile(id)
      .then(res => {
        commit('deleteFile', { id, type });
        return 'success';
      })
      .then(() => {
        dispatch('getAbout', {}, { root: true });
      });
  },
  batchDeleteFiles (
    { commit },
    { ids, type }: { ids: string[]; type: CollapseItemTabType }
  ): Promise<API_FILE.DriveBatchDeleteResponse[]> {
    return api.batchDeleteFiles(ids).then(res => {
      if (res.every(item => item.task_id !== undefined)) {
        FileTreeSqlManager.batchDeleteById(ids);
        commit('batchDeleteFiles', { ids, type });
      } else {
        throw new Error('删除失败');
      }
      return res;
    });
  },
  trashFile (
    { commit }: ActionContext<DriveState, DriveState>,
    { id, type }: { type: CollapseItemTabType; id: string }
  ) {
    return api.trashFile(id).then(res => {
      commit('deleteFile', { id, type });
      FileTreeSqlManager.batchTrashById([id]);
      return res;
    });
  },
  batchTrashFiles ({ commit }, { ids }: { ids: string[] }): Promise<API_FILE.DriveBatchTrashResponse[]> {
    return api.batchTrashFiles(ids).then(res => {
      if (res.every(item => item.task_id !== undefined)) {
        FileTreeSqlManager.batchTrashById(ids);
        commit('batchDeleteFiles', { ids, type: 'drive' });
      } else {
        throw new Error('删除失败');
      }
      return res;
    });
  },
  clearTrash ({ commit, dispatch }) {
    return clearTrash().then(res => {
      FileTreeSqlManager.deleteBy({ trashed: 1 });
      commit('setTrashFileList', { data: [], refresh: true });
    });
  },
  untrashFile (
    { commit }: ActionContext<DriveState, DriveState>,
    { id, type }: { id: string; type: CollapseItemTabType }
  ) {
    return api.untrashFile(id).then(res => {
      commit('deleteFile', { id, type });

      return res;
    });
  },
  batchUntrashFiles (
    { commit },
    { ids }: { ids: string[]; type: 'trash' }
  ): Promise<API_FILE.DriveBatchUntrashResponse[]> {
    return api.batchUntrashFiles(ids).then(res => {
      if (res.every(item => item.task_id !== undefined)) {
        FileTreeSqlManager.batchUnTrashById(ids);
        commit('batchDeleteFiles', { ids, type: 'trash' });
      } else {
        throw new Error('恢复失败');
      }
      return res;
    });
  },
  updateFile ({ commit }: ActionContext<DriveState, DriveState>, { id, params }: Dictionary) {
    return api.updateFile({ id, params }).then(res => {
      // if (!res.error) {
      //   alert('update success')
      // }
      // commit('set', res)
      return res;
    });
  },
  copyFile ({ commit, dispatch }: ActionContext<DriveState, DriveState>, { id, parent_id, name }: Dictionary) {
    return api
      .copyFile({ id, parent_id, name })
      .then(res => {
        if (Object.keys(res).length === 0) {
          console.log('copy success');
        }
        return res;
      })
      .then(() => {
        dispatch('getAbout', {}, { root: true });
      });
  },
  moveFile ({ commit, state }: ActionContext<DriveState, DriveState>, { id, parent_id, name }: Dictionary) {
    return api.moveFile({ id, parent_id, name }).then(() => {
      const list = state.driveFileList.filter(item => item.id !== id);
      commit('setDriveFileList', { data: list, refresh: true });
    });
  },
  // drive only
  batchMoveFile ({ commit, state }: ActionContext<DriveState, DriveState>, { fileIdList, parent_id }: Dictionary) {
    return batchMoveFiles(fileIdList, parent_id).then(res => {
      const list = state.driveFileList.filter(item => !fileIdList.includes(item.id));
      commit('setDriveFileList', { data: list, refresh: true });
      FileTreeSqlManager.batchUpdateParentId(fileIdList, parent_id);
      return res;
    });
  },
  // drive file only
  batchCopyFile (
    { commit, state, dispatch }: ActionContext<DriveState, DriveState>,
    { fileIdList, parent_id }: Dictionary
  ) {
    return batchCopyFiles(fileIdList, parent_id).then(res => {
      return res;
    });
  },

  delTasks ({ commit }: ActionContext<DriveState, DriveState>, ids: any) {
    return api.delTasks(ids).then(res => {
      if (res) {
        commit('delTasks', ids);
      }
    });
  },

  getTaskStatus ({ commit }: ActionContext<DriveState, DriveState>, { ids }: Dictionary) {
    const promiseIds = (chunk(ids, 20) as string[][]).map(_ids => {
      return api.getTasksStatus({ status_ids: _ids }).then(res => {
        if (res.statuses) {
          res.statuses.forEach((v: any) => {
            if (v) {commit('setAllTaskStatus', v);};
          });
          return _ids;
        }
      });
    });
    return Promise.all([promiseIds]).then(vals => vals.join());
  },
  getEvent ({ commit }: ActionContext<DriveState, DriveState>, params: Dictionary = {}) {
    return api.getEvent(params).then(res => {
      if (res) {
        commit('setEvents', {
          list: res.events,
          refresh: !params.page_token,
          pageToken: res.next_page_token
        });
      }
      console.log('getEvent', res);
    });
  },
  // addFolder (
  //   { commit, state }: ActionContext<DriveState, DriveState>,
  //   name: string
  // ) {
  //   return api
  //     .createFile({
  //       parent_id: state.parentIdMap.drive,
  //       name,
  //       kind: 'drive#folder'
  //     })
  //     .then(res => {
  //       return res
  //     })
  // },
  addFolderWithParentId (
    { commit, state },
    { parentId, name, ignore_duplicated_name = false }: { parentId: string; name: string; ignore_duplicated_name: boolean }
  ) {
    return api
      .createFile({
        parent_id: parentId,
        name,
        kind: 'drive#folder',
        ignore_duplicated_name
      })
      .then(res => {
        if (res.file) {
          FileTreeSqlManager.insert(res.file);
        }
        return res;
      });
  },
  getFolders ({ commit, state }: ActionContext<DriveState, DriveState>, params: any) {
    return api.findFiles(params).then(res => {
      if (!res.error) {
        const parentId: string | number = params.parent_id || '0';
        if (parentId !== '0') {
          commit('setFolders', {
            ...state.folders[parentId],
            isClick: true
          });
        }
        if (res.files.length === 0) {
          return;
        }
        const list = res.files.map((folder: { id: any; name: any }) => {
          commit('setFolders', folder);
          return {
            id: folder.id,
            name: folder.name
          };
        });
        commit('setFolderNodes', { parentId, list });
        return list;
      } else {
        alert('inter folder success');
      }
    });
  },

  applyPrivilege ({ commit }: ActionContext<DriveState, DriveState>, params: Record<string, any> | undefined) {
    return api.applyPrivilege(params);
  },
  delUploadTask ({ commit, state }: any, params: { name: any }) {
    const task = state.uploadList.filter((v: { name: any }) => v.name === params.name)[0];
    if (task && task.id) {
      return api.deleteFile(task.id).then(res => {
        console.log('任务彻底删除成功', res);
        commit('deleteFile', task.id);
        commit('delUploadTask', params);
      });
    } else {return null;};
  },
  setUploadTaskStatus ({ state }: ActionContext<DriveState, DriveState>, { name }: Dictionary) {
    const id = state.uploadList.filter((v: { name: any }) => v.name === name)[0].id;
    const params = { phase: 'PHASE_TYPE_RUNNING' };
    return api
      .updateFile({ id, params })
      .then(res => res)
      .catch(err => err);
  },
  checkImportDownload ({
    commit
  }: ActionContext<DriveState, DriveState>): Promise<API_FILE.DriveCheckImportDownloadResponse> {
    return api
      .checkImportDownload()
      .then(res => {
        commit('setImportStatus', res);
        return res;
      })
      .catch(err => {
        console.error(err);
        // 出错时将is_import设置为true，不展示弹窗
        const noShow = {
          is_import: true,
          can_load_records: 0
        };
        commit('setImportStatus', noShow);
        return noShow;
      });
  },
  // 供IPC调用的取回函数
  openFetchBackBW ({ commit, state }, { files, shareId = '', passCodeToken = '' }) {
    //
    // if (userId !== rootState.users.curUser.userId) {
    //   client.callRemoteClientFunction('ThunderPanPluginWebview', 'ShowToast', {
    //     message: '当前账号与网页端登录账号不一致，请切换账号后重试',
    //     type: 'error',
    //     position: 'middle',
    //     duration: 3000
    //   })
    // } else {
    // dispatch('fetchBackFiles', fileIds)
    // }
    commit('setShareInfo', { shareId, passCodeToken });
    client
      .checkRemoteFunction('ThunderPanPluginWebview', 'GetFetchBackFiles')
      .then(res => {
        if (!res) {
          client.registerFunctions({
            GetFetchBackFiles: () => state.toFetchBackList
          });
        }
        if (!isFetchBackBWOpen()) {
          commit('setFetchBackFiles', files);
          createFetchBackBW();
        } else {
          const shareFiles = state.toFetchBackList.concat(files);
          commit('setFetchBackFiles', shareFiles);
          callRemote(fetchBackContext, 'addMoreFile', files);
        }
      });
  },

  async fetchBackFiles (
    { rootState, state, commit, dispatch },
    files: createTaskInfo[]
  ) {
    // 获取全局配置，判断是否使用默认路径
    const useDefault = await getConfig('ThunderPanPlugin', 'useDefault', false);

    if (useDefault) {
      // 使用默认路径，直接拉起取回
      // 跳过文件夹大小的计算
      let totalFileSize = 0;
      for (const file of files) {
        if (file.kind === 'drive#file') { totalFileSize += Number(file.size) || 0; }
      }
      const defaultPath = (await getConfig(
        'ThunderPanPlugin',
        'defaultDownloadPath',
        ''
      )) as string;

      const freeSpace = getFreePartitionSpace(defaultPath);
      if (totalFileSize > freeSpace) {
        dispatch('openFetchBackBW', { files });
        return [];
      }
      dispatch('retrieval-list/startRetrieval', { files }, { root: true });
      eventTrack('transmission_getback_create', { from: 'yunpan_filelist' });
    } else {
      // 不使用默认路径，拉起弹窗
      // commit('setFetchBackFiles', files)
      // client
      //   .checkRemoteFunction('ThunderPanPluginWebview', 'GetFetchBackFiles')
      //   .then(res => {
      //     if (!res) {
      //       client.registerFunctions({
      //         GetFetchBackFiles: () => state.toFetchBackList
      //       })
      //     }
      //     if (!isFetchBackBWOpen()) {
      //       createFetchBackBW()
      //     } else {
      //       callRemote(fetchBackContext, 'addMoreFile', files)
      //     }
      //   })
      dispatch('openFetchBackBW', { files });
    }
  },

  async showFileDetail ({ commit, rootState }, options: IPlayerOption): Promise<API_FILE.DriveFile> {
    // const { fileId, isPlayInstantly, mode } = options
    if (options.redirect) {client.callServerFunction('SelectNav', NavViewID.Cloud);};
    if (options.redirect && rootState['media-player'].currentShowingFile.id !== options.fileId) {
      commit('media-player/setDisableXmpChangeMode', true, { root: true });
      await redirectCurrentDirectoryById(options.fileId);
      Vue.nextTick(() => {
        commit('media-player/setDisableXmpChangeMode', false, { root: true });
        // bus.$emit('setXmpPlayMode', 'panel')
        commit('media-player/setCurrentShowingFile', { id: options.fileId }, { root: true });
      });
    }
    const fileDetail = await api.getFileInfo(options.fileId);
    commit('media-player/setPlayerOption', {
      id: options.fileId,
      option: options
    }, {
      root: true
    });
    if (options.isPlayInstantly) {
      switch (checkCanPreview(fileDetail.file_extension || '')) {
        case 'media':
          bus.$emit('play', options.mode, options.playFrom);
          break;
        case 'picture':
          openImgView({ src: fileDetail.web_content_link || '' });
          break;
      }
    }
    return fileDetail;
  }
};

export type DriveState = {
  activeCollapseItemType: CollapseItemTabType;
  cover: boolean;
  all: Dictionary;
  home: {
    list: any[];
  };
  uploadList: any[];
  tasks: {
    list: any[];
    pageToken: string;
  };
  allTaskStatus: Dictionary;
  spaceMsg: {
    kind: string;
    limit: number;
    usage: number;
  };
  pageToken: string;
  hasMore: false;
  parentId: string;
  parentPaths: {
    id: string;
    name: string;
  }[]; // 目录路径
  parentIndex: number;
  // folders: {
  //   0: {
  //     id: '';
  //     name: '根目录';
  //   };
  // }; // 目录树
  driveFileList: API_FILE.DriveFile[];
  trashFileList: API_FILE.DriveFile[];
  folders: Dictionary;
  folderNodes: any[];
  events: {
    list: any[];
    pageToken: string;
  };
  uploadRetry: boolean;
  filterTypes: Dictionary;
  curFilter: string;
  nextPageTokenMap: Partial<LiteralKeyMap<CollapseItemTabType, string>>;
  parentIdMap: LiteralKeyMap<CollapseItemTabType, string>;
  selectItemListMap: LiteralKeyMap<CollapseItemTabType, API_FILE.DriveFile[]>;
  driveSortInfoMap: LiteralKeyMap<CollapseItemTabType, TypeDriveSortInfo>;
  driveRouteList: any[];
  importState: {
    isImported: boolean;
    canLoadRecords: number;
  };
  toFetchBackList: createTaskInfo[];
  shareInfo: {
    shareId: string;
    passCodeToken: string;
  };
  userFileTableColumnList: string[];
};

export type TaskInfo = {
  taskType: TaskType;
  taskBaseInfo: {
    taskName: string;
    savePath: string;
  };
  p2spTaskInfo: {
    nameFixed: 1; // 禁止传输库自动重命名
    url: string;
  };
  categoryId: string;
  fileId: string; // 保存在数据库，提供刷新链接用
  linkExpire: string; // 保存在数据库，提供刷新链接用
  token: string;
};
const a = (a, b, c) => {

};
const test = function result() {
  
};
type LiteralKeyMap<T extends string, S> = { [K in T]: S };
export type CollapseItemTabType = 'trash' | 'drive';
export type DriveMutations = typeof mutations;
export type TypeDriveSortInfo = {
  type: SortFunctionMapType;
  order: SortOrderType;
};


function Component() {

}
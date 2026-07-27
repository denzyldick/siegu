if vim.g.siegu_local_nvim_loaded then
  return
end

vim.g.siegu_local_nvim_loaded = true

local uv = vim.uv or vim.loop

local function path_join(...)
  return table.concat({ ... }, '/')
end

local function is_dir(path)
  local stat = uv.fs_stat(path)
  return stat and stat.type == 'directory'
end

local function find_root()
  local dir = uv.cwd()
  while dir and dir ~= '' do
    if uv.fs_stat(path_join(dir, 'package.json')) and is_dir(path_join(dir, 'src-tauri')) then
      return dir
    end
    local parent = vim.fn.fnamemodify(dir, ':h')
    if parent == dir then
      break
    end
    dir = parent
  end
  return uv.cwd()
end

local root = find_root()

local function open_terminal(title, cmd, cwd)
  vim.cmd('botright split')
  vim.cmd('resize 16')

  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_win_set_buf(0, buf)
  vim.bo[buf].bufhidden = 'wipe'
  vim.api.nvim_buf_set_name(buf, title)

  vim.fn.termopen(cmd, {
    cwd = cwd,
    env = {
      RUST_BACKTRACE = '1',
    },
  })

  vim.cmd('startinsert')
end

vim.api.nvim_create_user_command('SieguTauriDev', function()
  open_terminal('Siegu Tauri Dev', { 'npm', 'run', 'tauri', '--', 'dev' }, root)
end, {
  desc = 'Start the Tauri dev session for Siegu',
})

local function configure_dap()
  local ok, dap = pcall(require, 'dap')
  if not ok then
    return
  end

  if vim.fn.executable('codelldb') ~= 1 then
    return
  end

  dap.adapters.codelldb = {
    type = 'server',
    port = '${port}',
    executable = {
      command = 'codelldb',
      args = { '--port', '${port}' },
    },
  }

  dap.configurations.rust = dap.configurations.rust or {}

  local attach_config = {
    name = 'Siegu: attach to Tauri backend',
    type = 'codelldb',
    request = 'attach',
    cwd = path_join(root, 'src-tauri'),
    pid = function()
      local pids = vim.fn.systemlist({ 'pgrep', '-n', '-x', 'siegu' })
      if #pids == 0 then
        pids = vim.fn.systemlist({ 'pgrep', '-n', '-f', path_join('src-tauri', 'target', 'debug', 'siegu') })
      end
      local pid = pids[1]
      if not pid or pid == '' then
        error('Could not find a running Siegu backend process. Start :SieguTauriDev first.')
      end
      return tonumber(pid)
    end,
  }

  local launch_config = {
    name = 'Siegu: launch backend binary',
    type = 'codelldb',
    request = 'launch',
    cwd = path_join(root, 'src-tauri'),
    program = function()
      local exe = path_join(root, 'src-tauri', 'target', 'debug', 'siegu')
      if vim.fn.has('win32') == 1 then
        exe = exe .. '.exe'
      end
      return exe
    end,
    args = {},
    env = {
      RUST_BACKTRACE = '1',
      RUST_LOG = 'debug',
    },
  }

  local has_attach = false
  local has_launch = false
  for _, cfg in ipairs(dap.configurations.rust) do
    if cfg.name == attach_config.name then
      has_attach = true
    elseif cfg.name == launch_config.name then
      has_launch = true
    end
  end

  if not has_attach then
    table.insert(dap.configurations.rust, 1, attach_config)
  end
  if not has_launch then
    table.insert(dap.configurations.rust, 2, launch_config)
  end
end

configure_dap()

vim.api.nvim_create_user_command('SieguDebugAttach', function()
  local ok, dap = pcall(require, 'dap')
  if not ok then
    vim.notify('nvim-dap is not installed', vim.log.levels.ERROR)
    return
  end
  if vim.fn.executable('codelldb') ~= 1 then
    vim.notify('codelldb is not on PATH', vim.log.levels.ERROR)
    return
  end

  local pid = vim.fn.systemlist({ 'pgrep', '-n', '-x', 'siegu' })[1]
  if not pid or pid == '' then
    pid = vim.fn.systemlist({ 'pgrep', '-n', '-f', path_join('src-tauri', 'target', 'debug', 'siegu') })[1]
  end
  if not pid or pid == '' then
    vim.notify('No running Siegu backend process found. Start :SieguTauriDev first.', vim.log.levels.ERROR)
    return
  end

  dap.run({
    name = 'Siegu: attach to Tauri backend',
    type = 'codelldb',
    request = 'attach',
    pid = tonumber(pid),
    cwd = path_join(root, 'src-tauri'),
  })
end, {
  desc = 'Attach CodeLLDB to the running Siegu backend',
})

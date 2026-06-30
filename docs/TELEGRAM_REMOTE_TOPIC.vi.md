# Cau hinh Telegram Remote Download theo Topic

Tai lieu nay dung cho Youwee custom khi dat bot vao group Telegram co Topic.
Telegram Bot API ho tro `message_thread_id` trong Message va `sendMessage`:
https://core.telegram.org/bots/api

## BotFather

Ten goi y:

- Bot name: `Youwee Download Controller`
- Username: `YouweeDownloadControllerBot` hoac mot bien the con trong
- About: `Remote controller for Youwee downloads.`
- Description: `Send links or commands from Telegram to add, start, stop, and inspect Youwee downloads.`

Commands cho `/setcommands`:

```text
start - Show command keyboard
add - Add a URL to the queue
download - Add a URL and start downloading
status - Show download status
queue - Show recent queue items
run - Start pending downloads
stop - Stop current download
help - Show available commands
```

Privacy:

- Nen bat privacy mode neu bot dung chung group voi cac bot khac.
- Neu bat privacy mode, hay gui lenh dang `/status@BotUsername` hoac dung command keyboard.
- Chi tat privacy mode khi group/topic chi dung rieng cho Youwee va ban muon bot doc link thuong khong co slash command.

## Group va Topic

Ten group goi y: `Media Automation Control Center`

Topic goi y:

- `Youwee Downloads`
- `TikTok Live Recorder`
- `Logs & Alerts`

Voi link Telegram Web:

```text
https://web.telegram.org/a/#-1003775018720_360
```

Cau hinh trong Youwee:

```text
Allowed Chat IDs: -1003775018720
Topic message_thread_id: 360
```

Youwee van authorize theo `chat_id` cua group. Neu nhap `Topic message_thread_id`,
Youwee chi nhan command tu Topic do va reply lai dung Topic do. Neu de trong,
Youwee giu hanh vi cu va reply theo Topic cua message nhan duoc neu Telegram gui kem
`message_thread_id`.

## Xac nhan bang getUpdates

1. Them bot vao group `Media Automation Control Center`.
2. Mo Topic `Youwee Downloads`.
3. Gui mot lenh test, vi du `/status@BotUsername`.
4. Chay PowerShell:

```powershell
$token = Read-Host "Bot token"
$updates = Invoke-RestMethod "https://api.telegram.org/bot$token/getUpdates"
$updates.result | Select-Object `
  update_id, `
  @{Name="chat_id"; Expression={$_.message.chat.id}}, `
  @{Name="message_thread_id"; Expression={$_.message.message_thread_id}}, `
  @{Name="text"; Expression={$_.message.text}}
```

Ket qua dung se co:

```text
chat_id: -1003775018720
message_thread_id: 360
```

Neu `message_thread_id` trong ket qua khac `360`, hay dung gia tri thuc te Telegram tra ve cho Topic do.

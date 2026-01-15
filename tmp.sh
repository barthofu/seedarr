#!/bin/sh

API_BASE="https://yggisdead-api.bartho.dev/v1"
API_KEY="ti_xqFmjRabUdlk9iSHpdoWvrety2mHEB5YrshePltYens"

curl -X POST "$API_BASE/torrent/upload" \
  -H "Accept: application/json" \
  -H "Authorization: ApiKey $API_KEY" \
  -F "title=Test" \
  --form-string $'description=test\n\naaa' \
  -F "category=movies" \
  -F "tags=[]" \
  -F "torrent=@/mnt/nas/torrents/.seedarr/Kung.Fu.Panda.2008.MULTi.VF.1080p.BluRay.10bit.HDLight.AC3.x265-DeSs.torrent;type=application/x-bittorrent"
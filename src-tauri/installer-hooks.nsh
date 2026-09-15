; Récupération de la configuration du client, côté installeur Windows.
;
; Un seul launcher est compilé pour tous les clients. Le panel ajoute à la fin
; de cet installeur un bloc de 512 octets tenant sur deux lignes (voir
; `src/lib/provisioning.ts` du panel et `src-tauri/src/provisioning.rs`) :
;
;   LUUXCRAFT-PROVISIONING-V1<base64url>\n   à quel panel parler
;   LUUXCRAFT-BRAND-V1|<nom>|<url icône>|\n  sous quel nom s'installer
;
; La première ligne est recopiée telle quelle à côté du launcher, qui la lira au
; premier démarrage puis la recopiera dans son dossier de données : l'installeur
; ne peut pas la transmettre autrement à l'exécutable qu'il extrait. La seconde
; est lue ici, et sert à ce que le joueur retrouve *son* serveur dans le menu
; Démarrer et dans « Applications et fonctionnalités », pas le nom du launcher
; compilé.
;
; Pourquoi ce format : le bloc est de taille fixe pour que `FileSeek` puisse se
; placer à `-512` de la fin sans connaître la longueur de la charge utile, il
; est en ASCII imprimable parce que la conversion ANSI → UTF-16 d'un installeur
; Unicode est l'identité sur 0x20–0x7E quelle que soit la page de code, et il
; est découpé par des `\n` parce que `FileRead` s'arrête à chaque fin de ligne —
; un `FileRead` rend donc exactement une ligne.
;
; Tout échec est silencieux et sans conséquence : sans bloc lisible, le launcher
; demande son code au joueur au premier lancement et l'installation garde le nom
; compilé. Un installeur téléchargé par la mise à jour automatique, lui, n'a
; jamais de bloc — c'est normal, le launcher a déjà sa copie persistée.
;
; Ce hook est inséré à la toute fin de la section d'installation de tauri, donc
; après l'écriture du registre de désinstallation et après la création du
; raccourci du menu Démarrer : les retoucher ici est ce qui fait que nos valeurs
; gagnent. Le raccourci du Bureau, lui, est créé par la page finale *après* ce
; hook quand le joueur coche la case ; c'est le launcher qui le renomme à son
; premier démarrage.

!macro NSIS_HOOK_POSTINSTALL
    Push $0 ; descripteur de fichier
    Push $1 ; ligne 1 — provisionnement
    Push $2 ; ligne 2 — marque
    Push $3 ; nom du client
    Push $4 ; URL de l'icône du client
    Push $5 ; tampon de travail
    Push $6 ; index de lecture, puis dossier du raccourci
    Push $7 ; champ courant de la ligne de marque

    StrCpy $3 ""
    StrCpy $4 ""

    ClearErrors
    FileOpen $0 "$EXEPATH" r
    IfErrors luuxcraft_done

    ; 512 = PROVISIONING_FOOTER_SIZE. Doit rester synchronisé avec le panel.
    ; Les deux variables sont vidées d'abord : elles portent encore la valeur de
    ; l'appelant (elle est sur la pile, pas effacée), qu'une lecture ratée
    ; laisserait passer pour du contenu de bloc.
    FileSeek $0 -512 END
    StrCpy $1 ""
    ClearErrors
    FileRead $0 $1 512
    IfErrors luuxcraft_close_and_stop

    ; La ligne de marque est facultative — un client sans nom exploitable n'en a
    ; pas — et le remplissage qui la remplace alors n'a pas de fin de ligne :
    ; cette lecture-là touche la fin du fichier, ce qui est normal et ne doit
    ; surtout pas empêcher d'écrire le bloc de provisionnement.
    StrCpy $2 ""
    FileRead $0 $2 512
    FileClose $0
    ClearErrors
    Goto luuxcraft_lines_read

luuxcraft_close_and_stop:
    FileClose $0
    Goto luuxcraft_done

luuxcraft_lines_read:
    ; 25 = longueur de "LUUXCRAFT-PROVISIONING-V1".
    StrCpy $5 $1 25
    StrCmp $5 "LUUXCRAFT-PROVISIONING-V1" 0 luuxcraft_done

    ; Le bloc est recopié tel quel : c'est le launcher qui le décode, ce qui
    ; évite d'écrire un décodeur base64url en NSIS et garde une seule
    ; implémentation du format.
    ClearErrors
    FileOpen $0 "$INSTDIR\provisioning.blob" w
    IfErrors luuxcraft_brand
    FileWrite $0 $1
    FileClose $0
    DetailPrint "Configuration du serveur : OK"

luuxcraft_brand:
    ; --- découpe de "LUUXCRAFT-BRAND-V1|<nom>|<url>|" ---
    ;
    ; Un seul passage caractère par caractère, plutôt que WordFunc : la ligne
    ; fait moins de cent caractères, et ça évite une dépendance d'en-tête dans
    ; un fichier inséré au milieu d'une section.
    ClearErrors
    StrCpy $5 $2 18
    StrCmp $5 "LUUXCRAFT-BRAND-V1" 0 luuxcraft_done
    StrCpy $6 19 ; juste après "LUUXCRAFT-BRAND-V1|"
    StrCpy $7 1  ; 1 = nom, 2 = URL

luuxcraft_brand_loop:
    StrCpy $5 $2 1 $6
    StrCmp $5 "" luuxcraft_brand_ready
    StrCmp $5 "$\n" luuxcraft_brand_ready
    StrCmp $5 "$\r" luuxcraft_brand_ready
    IntOp $6 $6 + 1
    StrCmp $5 "|" 0 luuxcraft_brand_append
    IntOp $7 $7 + 1
    StrCmp $7 "3" luuxcraft_brand_ready luuxcraft_brand_loop

luuxcraft_brand_append:
    StrCmp $7 "1" 0 luuxcraft_brand_url
    StrCpy $3 "$3$5"
    Goto luuxcraft_brand_loop
luuxcraft_brand_url:
    StrCpy $4 "$4$5"
    Goto luuxcraft_brand_loop

luuxcraft_brand_ready:
    StrCmp $3 "" luuxcraft_done

    ; --- icône du client ---
    ;
    ; Téléchargée plutôt qu'embarquée dans le bloc : y faire tenir un fichier
    ; binaire demanderait de le relire octet par octet en NSIS, pour un résultat
    ; moins fiable. L'échec est sans conséquence — le launcher réécrit ce même
    ; fichier à chaque démarrage, l'icône n'est alors simplement correcte qu'à
    ; partir du premier lancement.
    StrCmp $4 "" luuxcraft_shortcut
    DetailPrint "Logo du serveur..."
    nsExec::ExecToLog "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command $\"try { [Net.ServicePointManager]::SecurityProtocol = 3072; (New-Object Net.WebClient).DownloadFile('$4', '$INSTDIR\brand.ico') } catch { }$\""
    Pop $5

luuxcraft_shortcut:
    StrCpy $5 ""
    IfFileExists "$INSTDIR\brand.ico" 0 +2
    StrCpy $5 "$INSTDIR\brand.ico"

    ; Le raccourci que tauri vient de créer porte le nom du produit compilé. On
    ; le remplace par un raccourci au nom du client, au même endroit — tauri en
    ; choisit un selon que le build définit un dossier de menu Démarrer, donc on
    ; cherche celui qui existe vraiment plutôt que de refaire ce choix ici.
    StrCpy $6 ""
    IfFileExists "$SMPROGRAMS\${PRODUCTNAME}.lnk" 0 luuxcraft_shortcut_folder
    StrCpy $6 "$SMPROGRAMS"
    Goto luuxcraft_shortcut_found
luuxcraft_shortcut_folder:
    IfFileExists "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" 0 luuxcraft_registry
    StrCpy $6 "$SMPROGRAMS\$AppStartMenuFolder"

luuxcraft_shortcut_found:
    Delete "$6\${PRODUCTNAME}.lnk"
    CreateShortcut "$6\$3.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$5"
    ; Même identifiant d'application que le raccourci de tauri, pour que la
    ; barre des tâches regroupe l'épinglage et la fenêtre. `!ifmacrodef` parce
    ; que cette macro appartient au gabarit de tauri, pas à NSIS.
    !ifmacrodef SetLnkAppUserModelId
        !insertmacro SetLnkAppUserModelId "$6\$3.lnk"
    !endif
    ; Le désinstalleur de tauri ne connaît que le nom compilé : sans ce chemin
    ; mémorisé, notre raccourci survivrait à la désinstallation.
    WriteRegStr SHCTX "${UNINSTKEY}" "LuuxCraftShortcut" "$6\$3.lnk"

luuxcraft_registry:
    ; Écrasent ce que tauri vient d'écrire quelques lignes plus haut dans la
    ; section : c'est ce que le joueur lit dans « Applications et
    ; fonctionnalités ».
    WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "$3"
    StrCmp $5 "" luuxcraft_done
    WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$5$\""

luuxcraft_done:
    ClearErrors
    Pop $7
    Pop $6
    Pop $5
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
    Push $0

    ; Ces fichiers ne sont pas dans le manifeste de l'installeur (ils sont créés
    ; après coup) : sans ça, ils survivraient à la désinstallation.
    Delete "$INSTDIR\provisioning.blob"
    Delete "$INSTDIR\brand.ico"

    ; Le raccourci porte le nom du client, que le désinstalleur de tauri ne
    ; connaît pas — d'où le chemin relu ici, tel qu'il a été écrit à
    ; l'installation.
    ReadRegStr $0 SHCTX "${UNINSTKEY}" "LuuxCraftShortcut"
    StrCmp $0 "" luuxcraft_uninstall_done
    Delete "$0"
    DeleteRegValue SHCTX "${UNINSTKEY}" "LuuxCraftShortcut"

luuxcraft_uninstall_done:
    ClearErrors
    Pop $0
!macroend

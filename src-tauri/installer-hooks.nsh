; Récupération de la configuration du client, côté installeur Windows.
;
; Un seul launcher est compilé pour tous les clients. Le panel ajoute à la fin
; de cet installeur un bloc de 512 octets qui dit à quel panel parler (voir
; `src/lib/provisioning.ts` du panel et `src-tauri/src/provisioning.rs`).
; L'installeur ne peut pas transmettre ce bloc à l'exécutable qu'il extrait :
; il le relit donc lui-même et le dépose à côté du launcher, qui le lira au
; premier démarrage puis le recopiera dans son dossier de données.
;
; Pourquoi ce format : le bloc est de taille fixe pour que `FileSeek` puisse se
; placer à `-512` de la fin sans connaître la longueur de la charge utile, et il
; est en ASCII imprimable sans retour à la ligne parce que `FileRead` s'arrête
; au premier `\n` et parce que la conversion ANSI → UTF-16 d'un installeur
; Unicode est l'identité sur 0x20–0x7E, quelle que soit la page de code.
;
; Tout échec est silencieux et sans conséquence : sans bloc lisible, le
; launcher demande son code au joueur au premier lancement. Un installeur
; téléchargé par la mise à jour automatique, lui, n'a jamais de bloc — c'est
; normal, le launcher a déjà sa copie persistée.

!macro NSIS_HOOK_POSTINSTALL
    Push $0
    Push $1
    Push $2

    ClearErrors
    FileOpen $0 "$EXEPATH" r
    IfErrors luuxcraft_provisioning_done

    ; 512 = PROVISIONING_FOOTER_SIZE. Doit rester synchronisé avec le panel.
    FileSeek $0 -512 END
    FileRead $0 $1 512
    FileClose $0
    IfErrors luuxcraft_provisioning_done

    ; 25 = longueur de "LUUXCRAFT-PROVISIONING-V1".
    StrCpy $2 $1 25
    StrCmp $2 "LUUXCRAFT-PROVISIONING-V1" 0 luuxcraft_provisioning_done

    ; Le bloc est recopié tel quel : c'est le launcher qui le décode, ce qui
    ; évite d'écrire un décodeur base64url en NSIS et garde une seule
    ; implémentation du format.
    ClearErrors
    FileOpen $2 "$INSTDIR\provisioning.blob" w
    IfErrors luuxcraft_provisioning_done
    FileWrite $2 $1
    FileClose $2
    DetailPrint "Configuration du serveur récupérée."

luuxcraft_provisioning_done:
    ClearErrors
    Pop $2
    Pop $1
    Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
    ; Le fichier n'est pas dans le manifeste de l'installeur (il est créé après
    ; coup) : sans ça, il survivrait à la désinstallation.
    Delete "$INSTDIR\provisioning.blob"
!macroend

import { Component, ViewChild } from '@angular/core'
import { PatchDbService } from 'src/app/services/patch-db/patch-db.service'
import {
  IonContent,
  LoadingController,
  ModalController,
  ToastController,
} from '@ionic/angular'
import {
  GenericInputComponent,
  GenericInputOptions,
} from 'src/app/modals/generic-input/generic-input.component'
import { ApiService } from 'src/app/services/api/embassy-api.service'
import { ServerConfigService } from 'src/app/services/server-config.service'
import { LocalStorageService } from '../../../services/local-storage.service'

@Component({
  selector: 'preferences',
  templateUrl: './preferences.page.html',
  styleUrls: ['./preferences.page.scss'],
})
export class PreferencesPage {
  @ViewChild(IonContent) content: IonContent
  defaultName: string
  clicks = 0

  constructor (
    private readonly loadingCtrl: LoadingController,
    private readonly modalCtrl: ModalController,
    private readonly api: ApiService,
    private readonly toastCtrl: ToastController,
    private readonly localStorageService: LocalStorageService,
    public readonly serverConfig: ServerConfigService,
    public readonly patch: PatchDbService,
  ) { }

  ngOnInit () {
    this.defaultName = `Embassy-${this.patch.getData()['server-info'].id}`
  }

  ngAfterViewInit () {
    this.content.scrollToPoint(undefined, 1)
  }

  async presentModalName (): Promise<void> {
    const options: GenericInputOptions = {
      title: 'Edit Device Name',
      message: 'This is for your reference only.',
      label: 'Device Name',
      useMask: false,
      placeholder: this.defaultName,
      nullable: true,
      initialValue: this.patch.getData().ui.name,
      buttonText: 'Save',
      submitFn: (value: string) =>
        this.setDbValue('name', value || this.defaultName),
    }

    const modal = await this.modalCtrl.create({
      componentProps: { options },
      cssClass: 'alertlike-modal',
      presentingElement: await this.modalCtrl.getTop(),
      component: GenericInputComponent,
    })

    await modal.present()
  }

  private async setDbValue (key: string, value: string): Promise<void> {
    const loader = await this.loadingCtrl.create({
      spinner: 'lines',
      message: 'Saving...',
      cssClass: 'loader',
    })
    await loader.present()

    try {
      await this.api.setDbValue({ pointer: `/${key}`, value })
    } finally {
      loader.dismiss()
    }
  }

  async addClick () {
    this.clicks++
    if (this.clicks >= 5) {
      this.clicks = 0
      const newVal = await this.localStorageService.toggleShowDevTools()
      const toast = await this.toastCtrl.create({
        header: newVal ? 'Dev tools unlocked' : 'Dev tools hidden',
        position: 'bottom',
        duration: 1000,
      })

      await toast.present()
    }
    setTimeout(() => {
      this.clicks = Math.max(this.clicks - 1, 0)
    }, 10000)
  }
}
